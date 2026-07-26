use crate::ipc_types::CommandError;
use orchestration::backend::{AgentDefinitionSummary, AppBackend};
use orchestration::{AgentDefinition, AppSettings, SkillSummary};

#[tauri::command]
pub fn list_agents(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Vec<AgentDefinitionSummary>, CommandError> {
    Ok(backend.list_agents()?)
}

#[tauri::command]
pub fn list_skills(backend: tauri::State<AppBackend>) -> Result<Vec<SkillSummary>, CommandError> {
    Ok(backend.list_skills()?)
}

#[tauri::command]
pub fn load_agents(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<AgentDefinition>, CommandError> {
    Ok(backend.load_agents()?)
}

#[tauri::command]
pub fn create_agent_definition(
    backend: tauri::State<AppBackend>,
    name: String,
) -> Result<AgentDefinition, CommandError> {
    Ok(backend.create_agent_definition(name)?)
}

#[tauri::command]
pub async fn create_agent_definition_with_ai(
    backend: tauri::State<'_, AppBackend>,
    description: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<AgentDefinition, CommandError> {
    Ok(backend
        .create_agent_definition_with_ai(description, &settings, transient_api_key.as_deref())
        .await?)
}

#[tauri::command]
pub fn save_agents(
    backend: tauri::State<AppBackend>,
    agents: Vec<AgentDefinition>,
) -> Result<(), CommandError> {
    Ok(backend.save_agents(&agents)?)
}
