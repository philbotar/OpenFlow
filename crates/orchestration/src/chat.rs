use crate::error::BackendError;
use engine::ApprovalMode;
use serde::{Deserialize, Serialize};
use std::io;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatConfig {
    pub model: Option<String>,
    pub approval_mode: ApprovalMode,
    pub reasoning_effort: Option<String>,
    pub reasoning_budget_tokens: Option<u32>,
    #[serde(default)]
    pub fast_mode: bool,
    pub project_id: Option<String>,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            model: None,
            approval_mode: ApprovalMode::ReadOnly,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            fast_mode: false,
            project_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub config: ChatConfig,
    pub run_id: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub trait ChatStore: Send + Sync {
    fn load(&self) -> io::Result<Vec<Chat>>;
    fn save(&self, chats: &[Chat]) -> io::Result<()>;
}

pub struct ChatCatalog {
    store: Box<dyn ChatStore>,
    mutation_lock: parking_lot::Mutex<()>,
}

impl ChatCatalog {
    #[must_use]
    pub fn new(store: Box<dyn ChatStore>) -> Self {
        Self {
            store,
            mutation_lock: parking_lot::Mutex::new(()),
        }
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or written.
    pub fn create(&self) -> Result<Chat, BackendError> {
        let _guard = self.mutation_lock.lock();
        let mut chats = self.store.load()?;
        let id = format!("chat-{}", Uuid::new_v4());
        let now_ms = chrono::Utc::now().timestamp_millis();
        let chat = Chat {
            id,
            title: "New chat".to_string(),
            config: ChatConfig::default(),
            run_id: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        chats.push(chat.clone());
        self.store.save(&chats)?;
        Ok(chat)
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read.
    pub fn list(&self) -> Result<Vec<Chat>, BackendError> {
        let mut chats = self.store.load()?;
        chats.sort_by_key(|chat| std::cmp::Reverse(chat.updated_at_ms));
        Ok(chats)
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or the chat does not exist.
    pub fn load_one(&self, chat_id: &str) -> Result<Chat, BackendError> {
        self.store
            .load()?
            .into_iter()
            .find(|chat| chat.id == chat_id)
            .ok_or_else(|| BackendError::ChatNotFound(chat_id.to_string()))
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or written, or the chat does not exist.
    pub fn delete(&self, chat_id: &str) -> Result<(), BackendError> {
        let _guard = self.mutation_lock.lock();
        let mut chats = self.store.load()?;
        let index = chats
            .iter()
            .position(|chat| chat.id == chat_id)
            .ok_or_else(|| BackendError::ChatNotFound(chat_id.to_string()))?;
        chats.remove(index);
        self.store.save(&chats)?;
        Ok(())
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or the chat does not exist.
    pub fn prepare_start(
        &self,
        chat_id: &str,
        first_message: Option<&str>,
    ) -> Result<Chat, BackendError> {
        let mut chat = self.load_one(chat_id)?;
        if chat.title == "New chat" {
            if let Some(title) = first_message.and_then(chat_title_from_message) {
                chat.title = title;
            }
        }
        Ok(chat)
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or written, or the chat does not exist.
    pub fn attach_run(&self, chat_id: &str, run_id: String) -> Result<Chat, BackendError> {
        self.attach_run_with_title(chat_id, None, run_id)
    }

    /// Persist the prepared title and run ID as one chat mutation.
    ///
    /// # Errors
    /// Returns an error if the chat store cannot be read or written, or the chat does not exist.
    pub fn attach_run_with_title(
        &self,
        chat_id: &str,
        title: Option<String>,
        run_id: String,
    ) -> Result<Chat, BackendError> {
        self.update(chat_id, |chat| {
            if let Some(title) = title {
                chat.title = title;
            }
            chat.run_id = Some(run_id);
        })
    }

    /// # Errors
    /// Returns an error if the chat store cannot be read or written, or the chat does not exist.
    pub fn update_config(&self, chat_id: &str, config: ChatConfig) -> Result<Chat, BackendError> {
        self.update(chat_id, |chat| {
            chat.config = config;
        })
    }

    fn update(&self, chat_id: &str, mutate: impl FnOnce(&mut Chat)) -> Result<Chat, BackendError> {
        let _guard = self.mutation_lock.lock();
        let mut chats = self.store.load()?;
        let chat = chats
            .iter_mut()
            .find(|chat| chat.id == chat_id)
            .ok_or_else(|| BackendError::ChatNotFound(chat_id.to_string()))?;
        mutate(chat);
        chat.updated_at_ms = chrono::Utc::now().timestamp_millis();
        let updated = chat.clone();
        self.store.save(&chats)?;
        Ok(updated)
    }
}

fn chat_title_from_message(message: &str) -> Option<String> {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    let mut chars = normalized.chars();
    let title = chars.by_ref().take(60).collect::<String>();
    Some(if chars.next().is_some() {
        format!("{title}…")
    } else {
        title
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::storage::chat_store::FileChatStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tempfile::tempdir;

    struct TrackingChatStore {
        chats: Mutex<Vec<Chat>>,
        active_calls: AtomicUsize,
        max_active_calls: AtomicUsize,
    }

    impl TrackingChatStore {
        fn enter(&self) {
            let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active_calls.fetch_max(active, Ordering::SeqCst);
            thread::sleep(std::time::Duration::from_millis(1));
        }

        fn leave(&self) {
            self.active_calls.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl ChatStore for Arc<TrackingChatStore> {
        fn load(&self) -> io::Result<Vec<Chat>> {
            self.enter();
            let chats = self.chats.lock().expect("chat store lock").clone();
            self.leave();
            Ok(chats)
        }

        fn save(&self, chats: &[Chat]) -> io::Result<()> {
            self.enter();
            *self.chats.lock().expect("chat store lock") = chats.to_vec();
            self.leave();
            Ok(())
        }
    }

    #[test]
    fn prepare_start_only_previews_title_until_run_attachment() {
        let dir = tempdir().expect("tempdir");
        let catalog = ChatCatalog::new(Box::new(FileChatStore::new(dir.path().join("chats.json"))));
        let created = catalog.create().expect("create chat");

        let prepared = catalog
            .prepare_start(&created.id, Some("Preview title"))
            .expect("prepare chat");

        assert_eq!(prepared.title, "Preview title");
        assert_eq!(
            catalog.load_one(&created.id).expect("load chat").title,
            "New chat"
        );
    }

    #[test]
    fn saved_chat_without_fast_mode_defaults_to_standard_speed() {
        let config: ChatConfig = serde_json::from_value(serde_json::json!({
            "model": "gpt-5.4",
            "approvalMode": "read_only",
            "reasoningEffort": "high",
            "reasoningBudgetTokens": null,
            "projectId": null
        }))
        .expect("deserialize legacy chat config");

        assert!(!config.fast_mode);
        assert_eq!(config.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn chat_mutations_are_serialized_across_threads() {
        let store = Arc::new(TrackingChatStore {
            chats: Mutex::new(Vec::new()),
            active_calls: AtomicUsize::new(0),
            max_active_calls: AtomicUsize::new(0),
        });
        let catalog = Arc::new(ChatCatalog::new(Box::new(Arc::clone(&store))));

        thread::scope(|scope| {
            for _ in 0..8 {
                let catalog = Arc::clone(&catalog);
                scope.spawn(move || catalog.create().expect("create chat"));
            }
        });

        assert_eq!(store.chats.lock().expect("chat store lock").len(), 8);
        assert_eq!(store.max_active_calls.load(Ordering::SeqCst), 1);
    }
}
