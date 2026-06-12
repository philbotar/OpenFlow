use crate::adapters::storage::json_file_store::{
    read_json_file, write_json_file, OPENFLOW_DATA_DIR_SLUG,
};
use crate::agent::ports::AgentStore;
use engine::CallableAgent;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

const AGENTS_FILE_NAME: &str = "agents.json";

#[derive(Debug, Clone)]
pub struct FileAgentStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct StoredAgents {
    agents: Vec<CallableAgent>,
}

impl FileAgentStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&self) -> io::Result<Vec<CallableAgent>> {
        let stored: StoredAgents =
            read_json_file(&self.path, "agent store JSON invalid")?.unwrap_or_default();
        Ok(stored.agents)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    pub fn save(&self, agents: &[CallableAgent]) -> io::Result<()> {
        let stored = StoredAgents {
            agents: agents.to_vec(),
        };
        write_json_file(&self.path, &stored, "agent store JSON")
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AgentStore for FileAgentStore {
    fn load(&self) -> io::Result<Vec<CallableAgent>> {
        FileAgentStore::load(self)
    }

    fn save(&self, agents: &[CallableAgent]) -> io::Result<()> {
        FileAgentStore::save(self, agents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::AgentNodeConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_store_loads_empty() {
        let dir = tempdir().unwrap();
        let store = FileAgentStore::new(dir.path().join(AGENTS_FILE_NAME));

        let agents = store.load().unwrap();

        assert!(agents.is_empty());
    }

    #[test]
    fn default_path_uses_openflow_slug() {
        let path = FileAgentStore::default_path();

        assert_eq!(path.file_name().unwrap(), AGENTS_FILE_NAME);
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            OPENFLOW_DATA_DIR_SLUG
        );
    }

    #[test]
    fn saves_and_loads_agents() {
        let dir = tempdir().unwrap();
        let store = FileAgentStore::new(dir.path().join("nested").join(AGENTS_FILE_NAME));
        let agent = CallableAgent::new("Saved");

        store.save(std::slice::from_ref(&agent)).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![agent]);
    }

    #[test]
    fn new_agent_uses_core_default_template() {
        let agent = CallableAgent::new("Templated");
        let defaults = AgentNodeConfig::default();

        assert_eq!(agent.system_prompt, defaults.system_prompt);
        assert_eq!(agent.task_prompt, defaults.task_prompt);
        assert_eq!(agent.model, defaults.model);
        assert_eq!(agent.output_schema, defaults.output_schema);
        assert_eq!(agent.auto_start, defaults.auto_start);
        assert_eq!(agent.tools, defaults.tools);
    }

    #[test]
    fn agent_definition_serde_backfills_tool_defaults() {
        let agent: CallableAgent = serde_json::from_value(serde_json::json!({
            "id": "agent-1",
            "name": "Legacy",
            "outputSchema": { "type": "object" }
        }))
        .unwrap();

        assert_eq!(agent.tools, engine::NodeToolConfig::default());
        assert!(!agent.auto_start);
    }
    #[test]
    fn agent_definition_deserializes_snake_case_fields() {
        let agent: CallableAgent = serde_json::from_value(serde_json::json!({
            "id": "agent-1",
            "name": "Snake",
            "system_prompt": "system",
            "task_prompt": "task",
            "model": "gpt-test",
            "output_schema": { "type": "object" },
            "auto_start": true,
            "tools": {
                "catalog": {
                    "tools": [{ "name": "read" }]
                },
                "approvalMode": "write",
                "overrides": []
            }
        }))
        .unwrap();

        assert_eq!(agent.system_prompt, "system");
        assert_eq!(agent.task_prompt, "task");
        assert_eq!(agent.model, "gpt-test");
        assert_eq!(agent.output_schema, serde_json::json!({ "type": "object" }));
        assert!(agent.auto_start);
        assert_eq!(agent.tools.catalog.tools[0].name, "read");
        assert_eq!(agent.tools.approval_mode, Some(engine::ApprovalMode::Write));
    }

    #[test]
    fn agent_definition_serializes_snake_case_fields() {
        let agent = CallableAgent::new("Serialized");
        let value = serde_json::to_value(&agent).unwrap();

        assert!(value.get("system_prompt").is_some());
        assert!(value.get("task_prompt").is_some());
        assert!(value.get("output_schema").is_some());
        assert!(value.get("auto_start").is_some());
        assert!(value.get("systemPrompt").is_none());
        assert!(value.get("taskPrompt").is_none());
        assert!(value.get("outputSchema").is_none());
        assert!(value.get("autoStart").is_none());
    }

    #[test]
    fn invalid_store_json_returns_invalid_data_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(AGENTS_FILE_NAME);
        fs::write(&path, "{\"agents\":").unwrap();
        let store = FileAgentStore::new(path);

        let error = store.load().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("agent store JSON invalid"));
    }

    #[test]
    fn saved_file_uses_agents_wrapper_key() {
        let dir = tempdir().unwrap();
        let store = FileAgentStore::new(dir.path().join(AGENTS_FILE_NAME));
        let agent = CallableAgent::new("Wrapped");

        store.save(std::slice::from_ref(&agent)).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert!(parsed.get("agents").is_some());
        assert_eq!(parsed["agents"][0]["name"], "Wrapped");
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(AGENTS_FILE_NAME);
        let store = FileAgentStore::new(&path);
        let agent = CallableAgent::new("Atomic");

        store.save(std::slice::from_ref(&agent)).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }
}
