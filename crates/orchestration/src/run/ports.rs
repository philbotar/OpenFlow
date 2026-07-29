use crate::run::persistence::{
    RunCheckpointPayload, RunRecord, RunStatus, RunStoreRoot, RunSummary,
};
use engine::ChatAttachmentRef;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const MAX_CHAT_ATTACHMENTS: usize = 4;
pub const MAX_CHAT_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_CHAT_ATTACHMENT_TOTAL_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentPreview {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAttachment {
    pub token: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub kind: engine::ChatAttachmentKind,
}

#[derive(Debug, Error)]
pub enum AttachmentError {
    #[error("A message can include at most {max} attachments.")]
    TooMany { max: usize },
    #[error("{file_name} is empty.")]
    Empty { file_name: String },
    #[error("{file_name} exceeds the {max_mib} MiB per-file attachment limit.")]
    FileTooLarge { file_name: String, max_mib: u64 },
    #[error("Attachments exceed the {max_mib} MiB total message limit.")]
    TotalTooLarge { max_mib: u64 },
    #[error("{file_name} is not a supported attachment: {reason}")]
    InvalidSource {
        file_name: String,
        reason: &'static str,
    },
    #[error("{file_name} has an unsupported file type.")]
    UnsupportedType { file_name: String },
    #[error("{file_name} content does not match its file type.")]
    TypeMismatch { file_name: String },
    #[error("{file_name} is missing or corrupt.")]
    Corrupt { file_name: String },
    #[error("Attachment storage failed for {file_name}: {detail}")]
    Storage { file_name: String, detail: String },
}

pub trait RunAttachmentStore: Send + Sync {
    fn stage(&self, file_name: &str, bytes: &[u8]) -> Result<StagedAttachment, AttachmentError>;
    fn remove_staged(&self, token: &str) -> Result<(), AttachmentError>;
    fn ingest_batch(
        &self,
        attachment_root: &Path,
        source_paths: &[PathBuf],
    ) -> Result<Vec<ChatAttachmentRef>, AttachmentError>;
    fn read(
        &self,
        attachment_root: &Path,
        attachment: &ChatAttachmentRef,
    ) -> Result<Vec<u8>, AttachmentError>;
    fn preview(
        &self,
        attachment_root: &Path,
        attachment: &ChatAttachmentRef,
    ) -> Result<AttachmentPreview, AttachmentError>;
    fn remove_batch(
        &self,
        attachment_root: &Path,
        attachments: &[ChatAttachmentRef],
    ) -> Result<(), AttachmentError>;
}

pub trait RunCheckpointStore: Send + Sync {
    fn create_run(&self, root: &RunStoreRoot, record: &RunRecord) -> io::Result<()>;
    fn append_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        payload: &RunCheckpointPayload,
    ) -> io::Result<()>;
    fn load_record(
        &self,
        roots: &[RunStoreRoot],
        run_id: &str,
    ) -> io::Result<Option<(RunStoreRoot, RunRecord)>>;
    fn load_latest_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
    ) -> io::Result<Option<RunCheckpointPayload>>;
    fn list_runs(
        &self,
        roots: &[RunStoreRoot],
        workflow_id: Option<&str>,
    ) -> io::Result<Vec<RunSummary>>;
    fn update_status(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        status: RunStatus,
        updated_at_ms: i64,
    ) -> io::Result<()>;
    fn run_dir(&self, root: &RunStoreRoot, run_id: &str) -> PathBuf;
    fn remove_run(&self, root: &RunStoreRoot, run_id: &str) -> io::Result<()>;
    fn quarantine_run(&self, root: &RunStoreRoot, run_id: &str) -> io::Result<Option<PathBuf>>;
    fn restore_quarantined_run(
        &self,
        quarantine_path: &Path,
        root: &RunStoreRoot,
        run_id: &str,
    ) -> io::Result<()>;
    fn remove_quarantined_run(&self, quarantine_path: &Path) -> io::Result<()>;
}
