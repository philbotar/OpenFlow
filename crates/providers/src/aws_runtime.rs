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
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(trimmed_region.to_string()));
    if let Some(name) = profile.map(str::trim).filter(|value| !value.is_empty()) {
        loader = loader.profile_name(name);
    }
    loader.load().await
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

    fn restore_env_var(name: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}
