use domain::AgentNodeConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CURRENT_DATA_DIR_SLUG: &str = "openflow";
const LEGACY_DATA_DIR_SLUG: &str = "step-through-agentic-workflow";
const AGENTS_FILE_NAME: &str = "agents.json";

fn legacy_store_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let dir_name = parent.file_name()?;

    if dir_name != CURRENT_DATA_DIR_SLUG || path.file_name()? != AGENTS_FILE_NAME {
        return None;
    }

    Some(
        parent
            .with_file_name(LEGACY_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME),
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    #[serde(default, alias = "systemPrompt")]
    pub system_prompt: String,
    #[serde(default, alias = "taskPrompt")]
    pub task_prompt: String,
    #[serde(default)]
    pub model: String,
    #[serde(alias = "outputSchema")]
    pub output_schema: serde_json::Value,
    #[serde(default, alias = "autoStart")]
    pub auto_start: bool,
    #[serde(default)]
    pub tools: domain::NodeToolConfig,
}

impl AgentDefinition {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let defaults = AgentNodeConfig::default();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            system_prompt: defaults.system_prompt,
            task_prompt: defaults.task_prompt,
            model: defaults.model,
            output_schema: defaults.output_schema,
            auto_start: defaults.auto_start,
            tools: defaults.tools,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileAgentStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct StoredAgents {
    agents: Vec<AgentDefinition>,
}

impl FileAgentStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CURRENT_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&self) -> io::Result<Vec<AgentDefinition>> {
        let path = if self.path.exists() {
            self.path.clone()
        } else if let Some(legacy_path) = legacy_store_path(&self.path) {
            if legacy_path.exists() {
                legacy_path
            } else {
                return Ok(Vec::new());
            }
        } else {
            return Ok(Vec::new());
        };

        let text = fs::read_to_string(&path)?;
        let stored: StoredAgents = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("agent store JSON invalid: {error}"),
            )
        })?;
        Ok(stored.agents)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    pub fn save(&self, agents: &[AgentDefinition]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let stored = StoredAgents {
            agents: agents.to_vec(),
        };
        let text = serde_json::to_string_pretty(&stored).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("agent store JSON serialization failed: {error}"),
            )
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            CURRENT_DATA_DIR_SLUG
        );
    }

    #[test]
    fn saves_and_loads_agents() {
        let dir = tempdir().unwrap();
        let store = FileAgentStore::new(dir.path().join("nested").join(AGENTS_FILE_NAME));
        let agent = AgentDefinition::new("Saved");

        store.save(std::slice::from_ref(&agent)).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![agent]);
    }

    #[test]
    fn new_agent_uses_core_default_template() {
        let agent = AgentDefinition::new("Templated");
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
        let agent: AgentDefinition = serde_json::from_value(serde_json::json!({
            "id": "agent-1",
            "name": "Legacy",
            "outputSchema": { "type": "object" }
        }))
        .unwrap();

        assert_eq!(agent.tools, domain::NodeToolConfig::default());
        assert!(!agent.auto_start);
    }
    #[test]
    fn agent_definition_deserializes_snake_case_fields() {
        let agent: AgentDefinition = serde_json::from_value(serde_json::json!({
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
                "maxToolRounds": 2,
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
        assert_eq!(agent.tools.approval_mode, Some(domain::ApprovalMode::Write));
        assert_eq!(agent.tools.max_tool_rounds, 2);
    }

    #[test]
    fn agent_definition_serializes_snake_case_fields() {
        let agent = AgentDefinition::new("Serialized");
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
    fn load_uses_legacy_store_when_new_path_is_absent() {
        let dir = tempdir().unwrap();
        let current_path = dir
            .path()
            .join(CURRENT_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME);
        let legacy_path = dir.path().join(LEGACY_DATA_DIR_SLUG).join(AGENTS_FILE_NAME);
        let agent = AgentDefinition::new("Legacy");
        let stored = StoredAgents {
            agents: vec![agent.clone()],
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, serde_json::to_string_pretty(&stored).unwrap()).unwrap();
        let store = FileAgentStore::new(current_path);

        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![agent]);
    }

    #[test]
    fn load_prefers_new_store_when_both_paths_exist() {
        let dir = tempdir().unwrap();
        let current_path = dir
            .path()
            .join(CURRENT_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME);
        let legacy_path = dir.path().join(LEGACY_DATA_DIR_SLUG).join(AGENTS_FILE_NAME);
        let current_agent = AgentDefinition::new("Current");
        let legacy_agent = AgentDefinition::new("Legacy");
        let current_stored = StoredAgents {
            agents: vec![current_agent.clone()],
        };
        let legacy_stored = StoredAgents {
            agents: vec![legacy_agent],
        };
        fs::create_dir_all(current_path.parent().unwrap()).unwrap();
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(
            &current_path,
            serde_json::to_string_pretty(&current_stored).unwrap(),
        )
        .unwrap();
        fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_stored).unwrap(),
        )
        .unwrap();
        let store = FileAgentStore::new(current_path);

        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![current_agent]);
    }

    #[test]
    fn save_targets_new_path_even_when_legacy_store_exists() {
        let dir = tempdir().unwrap();
        let current_path = dir
            .path()
            .join(CURRENT_DATA_DIR_SLUG)
            .join(AGENTS_FILE_NAME);
        let legacy_path = dir.path().join(LEGACY_DATA_DIR_SLUG).join(AGENTS_FILE_NAME);
        let legacy_agent = AgentDefinition::new("Legacy");
        let new_agent = AgentDefinition::new("Current");
        let legacy_stored = StoredAgents {
            agents: vec![legacy_agent.clone()],
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_stored).unwrap(),
        )
        .unwrap();
        let store = FileAgentStore::new(&current_path);

        store.save(std::slice::from_ref(&new_agent)).unwrap();

        let current_raw = fs::read_to_string(&current_path).unwrap();
        let current_stored: StoredAgents = serde_json::from_str(&current_raw).unwrap();
        let legacy_raw = fs::read_to_string(&legacy_path).unwrap();
        let legacy_stored_after: StoredAgents = serde_json::from_str(&legacy_raw).unwrap();

        assert_eq!(current_stored.agents, vec![new_agent]);
        assert_eq!(legacy_stored_after.agents, vec![legacy_agent]);
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
        let agent = AgentDefinition::new("Wrapped");

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
        let agent = AgentDefinition::new("Atomic");

        store.save(std::slice::from_ref(&agent)).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }
}
