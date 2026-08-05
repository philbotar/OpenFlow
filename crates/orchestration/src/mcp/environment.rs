use std::ffi::OsString;
use std::process::Stdio;
use std::time::Duration;

const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(2);
const PATH_MARKER: &str = "__OPENFLOW_MCP_PATH__";

/// Resolves the shell a user expects for local command execution.
pub(crate) fn user_shell() -> OsString {
    if let Some(shell) = std::env::var_os("SHELL").filter(|shell| !shell.is_empty()) {
        return shell;
    }

    #[cfg(target_os = "macos")]
    {
        OsString::from("/bin/zsh")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        OsString::from("/bin/sh")
    }
    #[cfg(windows)]
    {
        std::env::var_os("COMSPEC").unwrap_or_else(|| OsString::from("cmd.exe"))
    }
}

/// Resolves the PATH a user gets from the login shell used by the embedded
/// terminal and local child commands. Desktop apps on macOS often start
/// without shell init files, so their inherited PATH can omit runtimes such
/// as nvm's Node.
pub async fn effective_path() -> Option<OsString> {
    let fallback = std::env::var_os("PATH");

    #[cfg(not(unix))]
    {
        return fallback;
    }

    #[cfg(unix)]
    {
        let shell = user_shell();
        let mut command = tokio::process::Command::new(shell);
        command
            .args(["-ilc", &format!("printf '\\n{PATH_MARKER}%s\\n' \"$PATH\"")])
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(LOGIN_SHELL_TIMEOUT, command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(_)) | Err(_) => return fallback,
        };
        parse_login_shell_path(&output.stdout).or(fallback)
    }
}

fn parse_login_shell_path(output: &[u8]) -> Option<OsString> {
    String::from_utf8_lossy(output)
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(PATH_MARKER))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(OsString::from)
}

#[cfg(test)]
mod tests {
    use super::parse_login_shell_path;
    use std::ffi::OsString;

    #[test]
    fn parse_login_shell_path_ignores_shell_startup_output() {
        let output = b"startup notice\n__OPENFLOW_MCP_PATH__/custom/bin:/usr/bin\n";

        assert_eq!(
            parse_login_shell_path(output),
            Some(OsString::from("/custom/bin:/usr/bin"))
        );
    }

    #[test]
    fn parse_login_shell_path_rejects_missing_marker() {
        assert_eq!(parse_login_shell_path(b"/usr/bin:/bin\n"), None);
    }
}
