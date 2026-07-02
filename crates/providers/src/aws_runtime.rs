use aws_sdk_bedrockruntime::config::{Credentials, ProvideCredentials};
use std::path::PathBuf;

/// Ensures `HOME` is set so the AWS SDK can resolve `~/.aws/config` and SSO token cache.
///
/// GUI apps launched from Dock/Spotlight/Linux desktop entries often inherit neither shell
/// env vars nor a reliable `HOME`, so `aws sts get-caller-identity` works in a terminal while
/// the in-process credential chain cannot find shared config files.
pub fn ensure_process_home_env() {
    if process_home_dir().is_some() {
        return;
    }
    if let Some(home) = resolve_home_for_aws() {
        // ponytail: GUI apps often lack HOME; aws-config needs it for ~/.aws
        std::env::set_var("HOME", home);
    }
}

#[must_use]
pub fn process_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(windows_home_dir)
}

#[allow(
    clippy::redundant_pub_crate,
    reason = "shared Bedrock loader used from bedrock_models sibling module"
)]
pub(crate) async fn load_aws_sdk_config(
    region: &str,
    profile: Option<&str>,
) -> aws_config::SdkConfig {
    ensure_process_home_env();
    let trimmed_region = region.trim();
    let profile_name = sanitize_profile(profile);
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(trimmed_region.to_string()));
    if let Some(name) = profile_name {
        loader = loader.profile_name(name);
    }
    let shared = loader.load().await;
    let chain_has_credentials = match shared.credentials_provider() {
        Some(provider) => provider.provide_credentials().await.is_ok(),
        None => false,
    };
    if chain_has_credentials {
        return shared;
    }
    // ponytail: probe-then-CLI-fallback runs once per client build; cache if latency matters
    let credentials = match cli_export_credentials(profile_name).await {
        Some(credentials) => Some(credentials),
        None => sso_login_and_retry(profile_name).await,
    };
    if let Some(credentials) = credentials {
        return aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(trimmed_region.to_string()))
            .credentials_provider(credentials)
            .load()
            .await;
    }
    shared
}

/// Parses `aws configure export-credentials --format process` JSON.
fn parse_cli_export_credentials(json: &[u8]) -> Option<Credentials> {
    let value: serde_json::Value = serde_json::from_slice(json).ok()?;
    Some(Credentials::new(
        value["AccessKeyId"].as_str()?.to_string(),
        value["SecretAccessKey"].as_str()?.to_string(),
        value["SessionToken"].as_str().map(str::to_string),
        None, // ponytail: no expiry — config is rebuilt per invoke, creds used immediately
        "aws-cli-export-credentials",
    ))
}

/// Fallback for profiles the Rust SDK credential chain cannot resolve
/// (IAM Identity Center support is partial in aws-config; the CLI handles all shapes).
async fn cli_export_credentials(profile: Option<&str>) -> Option<Credentials> {
    let mut command = tokio::process::Command::new("aws");
    command.kill_on_drop(true);
    command.args(["configure", "export-credentials", "--format", "process"]);
    if let Some(name) = profile {
        command.args(["--profile", name]);
    }
    // ponytail: 30s cap — export should be fast; kill_on_drop cleans up on timeout
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_cli_export_credentials(&output.stdout)
}

/// Runs `aws sso login` (browser flow) and retries the CLI credential export once.
/// Mirrors Claude Code's awsAuthRefresh behavior for expired SSO sessions.
async fn sso_login_and_retry(profile: Option<&str>) -> Option<Credentials> {
    let mut command = tokio::process::Command::new("aws");
    command.args(["sso", "login"]);
    if let Some(name) = profile {
        command.args(["--profile", name]);
    }
    // ponytail: 120s cap — browser flow needs human time but must not hang a run forever
    let mut child = command.spawn().ok()?;
    let Ok(status) = tokio::time::timeout(std::time::Duration::from_mins(2), child.wait()).await
    else {
        let _ = child.kill().await;
        return None;
    };
    let status = status.ok()?;
    if !status.success() {
        return None;
    }
    cli_export_credentials(profile).await
}

fn sanitize_profile(profile: Option<&str>) -> Option<&str> {
    profile.map(str::trim).filter(|name| !name.is_empty())
}

fn resolve_home_for_aws() -> Option<PathBuf> {
    process_home_dir().or_else(dirs::home_dir)
}

#[cfg(windows)]
fn windows_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            Some(PathBuf::from(drive).join(path))
        })
}

#[cfg(not(windows))]
const fn windows_home_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn ensure_process_home_env_sets_home_from_userprofile_on_windows() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous_home = std::env::var_os("HOME");
        let previous_userprofile = std::env::var_os("USERPROFILE");
        std::env::remove_var("HOME");
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", r"C:\Users\openflow-test");

        ensure_process_home_env();

        #[cfg(windows)]
        assert_eq!(
            std::env::var_os("HOME").as_deref(),
            Some(std::ffi::OsStr::new(r"C:\Users\openflow-test"))
        );
        #[cfg(not(windows))]
        assert!(std::env::var_os("HOME").is_some());

        restore_env_var("HOME", previous_home);
        restore_env_var("USERPROFILE", previous_userprofile);
    }

    #[test]
    fn sanitize_profile_trims_and_rejects_blank() {
        assert_eq!(sanitize_profile(Some("  my-profile  ")), Some("my-profile"));
        assert_eq!(sanitize_profile(Some("   ")), None);
        assert_eq!(sanitize_profile(Some("")), None);
        assert_eq!(sanitize_profile(None), None);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn parses_cli_export_credentials_json() {
        let json = br#"{"Version":1,"AccessKeyId":"AKIA1","SecretAccessKey":"secret","SessionToken":"tok","Expiration":"2099-01-01T00:00:00Z"}"#;
        let creds = parse_cli_export_credentials(json).expect("credentials");
        assert_eq!(creds.access_key_id(), "AKIA1");
        assert_eq!(creds.session_token(), Some("tok"));
        assert!(parse_cli_export_credentials(b"{}").is_none());
    }

    fn restore_env_var(name: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
