//! Process-group cleanup shared by shell tools and MCP package installs.

#[cfg(unix)]
pub(crate) struct ProcessGroupKillGuard {
    pgid: Option<i32>,
}

#[cfg(unix)]
impl ProcessGroupKillGuard {
    pub(crate) fn new(child: &tokio::process::Child) -> Self {
        Self {
            pgid: child.id().and_then(|pid| i32::try_from(pid).ok()),
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.pgid = None;
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupKillGuard {
    fn drop(&mut self) {
        let Some(pgid) = self.pgid else {
            return;
        };
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
    }
}

#[cfg(unix)]
pub(crate) async fn kill_process_group(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        let pgid = i32::try_from(pid).unwrap_or(i32::MAX);
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
    }
    let _ = child.kill().await;
}

#[cfg(not(unix))]
pub(crate) async fn kill_process_group(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
}
