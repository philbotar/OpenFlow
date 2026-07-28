use crate::adapters::storage::json_file_store::{
    read_json_file, write_json_file, OPENFLOW_DATA_DIR_SLUG,
};
use crate::chat::{Chat, ChatStore};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

const CHATS_FILE_NAME: &str = "chats.json";

#[derive(Debug, Clone)]
pub struct FileChatStore {
    path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredChats {
    chats: Vec<Chat>,
}

impl FileChatStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join(CHATS_FILE_NAME)
    }
}

impl ChatStore for FileChatStore {
    fn load(&self) -> io::Result<Vec<Chat>> {
        let stored: StoredChats =
            read_json_file(&self.path, "chat store JSON invalid")?.unwrap_or_default();
        Ok(stored.chats)
    }

    fn save(&self, chats: &[Chat]) -> io::Result<()> {
        write_json_file(
            &self.path,
            &StoredChats {
                chats: chats.to_vec(),
            },
            "chat store JSON",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatCatalog, ChatConfig};
    use tempfile::tempdir;

    #[test]
    fn created_chat_survives_store_reopen() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(CHATS_FILE_NAME);
        let catalog = ChatCatalog::new(Box::new(FileChatStore::new(&path)));

        let created = catalog.create().expect("create chat");
        let reopened = FileChatStore::new(path).load().expect("reopen chats");

        assert_eq!(reopened, vec![created]);
    }

    #[test]
    fn legacy_chat_without_config_loads_safe_defaults() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(CHATS_FILE_NAME);
        std::fs::write(
            &path,
            r#"{"chats":[{"id":"chat-1","title":"Legacy","runId":null,"createdAtMs":1,"updatedAtMs":1}]}"#,
        )
        .expect("write legacy chat");

        let chats = FileChatStore::new(path).load().expect("load legacy chat");

        assert_eq!(chats[0].config, ChatConfig::default());
    }
}
