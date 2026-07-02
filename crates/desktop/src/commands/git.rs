use crate::ipc_types::CommandError;
use orchestration::backend::BackendError;

#[tauri::command]
pub async fn git_diff_repo(cwd: String) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::diff_repo(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))?
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
}

#[tauri::command]
pub async fn git_is_repo(cwd: String) -> Result<bool, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::is_repo(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
}

#[tauri::command]
pub async fn git_current_branch(cwd: String) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::current_branch(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))?
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
}
