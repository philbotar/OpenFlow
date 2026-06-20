use crate::run::persistence::{
    RunCheckpointPayload, RunRecord, RunStatus, RunStoreRoot, RunSummary,
};
use std::io;
use std::path::PathBuf;

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
}
