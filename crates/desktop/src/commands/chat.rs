use crate::ipc_types::{ChatRunPayload, CommandError};
use crate::run_event_bridge::spawn_run_event_bridge;
use orchestration::backend::AppBackend;
use orchestration::{AppSettings, Chat, ChatConfig};

#[tauri::command]
pub fn create_chat(backend: tauri::State<AppBackend>) -> Result<Chat, CommandError> {
    Ok(backend.create_chat()?)
}

#[tauri::command]
pub fn list_chats(backend: tauri::State<AppBackend>) -> Result<Vec<Chat>, CommandError> {
    Ok(backend.list_chats()?)
}

#[tauri::command]
pub fn delete_chat(backend: tauri::State<AppBackend>, chat_id: String) -> Result<(), CommandError> {
    Ok(backend.delete_chat(&chat_id)?)
}

#[tauri::command]
pub fn update_chat_config(
    backend: tauri::State<AppBackend>,
    chat_id: String,
    config: ChatConfig,
) -> Result<Chat, CommandError> {
    Ok(backend.update_chat_config(&chat_id, config)?)
}

#[tauri::command]
pub async fn start_chat(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    chat_id: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
    entrypoint: String,
) -> Result<ChatRunPayload, CommandError> {
    let (chat, run_state, event_rx) = backend
        .start_chat(
            &chat_id,
            Some(entrypoint),
            &settings,
            transient_api_key.as_deref(),
        )
        .await?;
    spawn_run_event_bridge(app, chat.title.clone(), event_rx, run_state.run_id.clone());
    Ok(ChatRunPayload { chat, run_state })
}
