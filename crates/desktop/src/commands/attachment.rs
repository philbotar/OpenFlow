use crate::ipc_types::CommandError;
use orchestration::backend::{AppBackend, AttachmentPreviewPayload, StagedAttachmentPayload};

#[tauri::command]
pub async fn load_chat_attachment_preview(
    backend: tauri::State<'_, AppBackend>,
    run_id: String,
    attachment_id: String,
) -> Result<AttachmentPreviewPayload, CommandError> {
    Ok(backend
        .load_chat_attachment_preview(&run_id, &attachment_id)
        .await?)
}

#[tauri::command]
pub fn stage_chat_attachment(
    backend: tauri::State<'_, AppBackend>,
    file_name: String,
    media_type: Option<String>,
    data_base64: String,
) -> Result<StagedAttachmentPayload, CommandError> {
    drop(media_type);
    Ok(backend.stage_chat_attachment(&file_name, &data_base64)?)
}

#[tauri::command]
pub fn remove_staged_chat_attachment(
    backend: tauri::State<'_, AppBackend>,
    token: String,
) -> Result<(), CommandError> {
    Ok(backend.remove_staged_chat_attachment(&token)?)
}
