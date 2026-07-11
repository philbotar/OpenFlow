use std::path::PathBuf;

/// Path to the `search` sidecar copied next to the OpenFlow executable.
pub(crate) fn resolve_sidecar_path(name: &str) -> Option<PathBuf> {
    let exe = tauri::utils::platform::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let base = if exe_dir.ends_with("deps") {
        exe_dir.parent().unwrap_or(exe_dir)
    } else {
        exe_dir
    };
    let path = base.join(name);
    #[cfg(windows)]
    let path = {
        let mut path = path;
        if path.extension().is_none() {
            path.as_mut_os_string().push(".exe");
        }
        path
    };
    path.is_file().then_some(path)
}

pub(crate) fn publish_bundled_search_path() {
    let Some(path) = resolve_sidecar_path("search") else {
        return;
    };
    let _ = orchestration::tool::set_bundled_search_binary(path);
}
