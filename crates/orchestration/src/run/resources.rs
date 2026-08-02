//! Process-wide execution budgets shared by independent top-level runs.

use async_trait::async_trait;
use engine::{
    AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink, ApprovalMode, Workflow,
};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const DEFAULT_AI_CONCURRENCY: usize = 8;
const DEFAULT_TOOL_CONCURRENCY: usize = 16;

/// Shared process resources. Session registration remains unbounded; only active work queues.
pub(crate) struct SharedRunResources {
    ai: Arc<Semaphore>,
    tools: Arc<Semaphore>,
    mutation_gates: Mutex<BTreeMap<PathBuf, Arc<Semaphore>>>,
}

impl Default for SharedRunResources {
    fn default() -> Self {
        Self::with_limits(DEFAULT_AI_CONCURRENCY, DEFAULT_TOOL_CONCURRENCY)
    }
}

impl SharedRunResources {
    #[must_use]
    pub(crate) fn with_limits(ai: usize, tools: usize) -> Self {
        assert!(ai > 0, "AI concurrency must be greater than zero");
        assert!(tools > 0, "tool concurrency must be greater than zero");
        Self {
            ai: Arc::new(Semaphore::new(ai)),
            tools: Arc::new(Semaphore::new(tools)),
            mutation_gates: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) async fn acquire_ai(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.ai)
            .acquire_owned()
            .await
            .expect("shared AI semaphore remains open")
    }

    #[cfg(test)]
    pub(crate) async fn acquire_tool(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.tools)
            .acquire_owned()
            .await
            .expect("shared tool semaphore remains open")
    }

    #[must_use]
    pub(crate) fn tool_budget(&self) -> Arc<Semaphore> {
        Arc::clone(&self.tools)
    }

    #[must_use]
    pub(crate) fn mutation_gate_for(&self, execution_cwd: &Path) -> Arc<Semaphore> {
        let mut gates = self.mutation_gates.lock();
        Arc::clone(
            gates
                .entry(execution_cwd.to_path_buf())
                .or_insert_with(|| Arc::new(Semaphore::new(1))),
        )
    }

    #[must_use]
    pub(crate) fn mutation_gate_for_workflow(
        &self,
        workflow: &Workflow,
        execution_cwd: &Path,
    ) -> Option<Arc<Semaphore>> {
        workflow
            .nodes
            .iter()
            .any(|node| !matches!(node.agent.tools.approval_mode, Some(ApprovalMode::ReadOnly)))
            .then(|| self.mutation_gate_for(execution_cwd))
    }
}

/// Acquires one shared provider permit for the full request/stream lifetime.
pub(crate) struct BudgetedAiPort {
    inner: Box<dyn AiPort>,
    resources: Arc<SharedRunResources>,
}

impl BudgetedAiPort {
    #[must_use]
    pub(crate) fn new(inner: Box<dyn AiPort>, resources: Arc<SharedRunResources>) -> Self {
        Self { inner, resources }
    }
}

#[async_trait]
impl AiPort for BudgetedAiPort {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let _permit = self.resources.acquire_ai().await;
        self.inner.invoke(request).await
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let _permit = self.resources.acquire_ai().await;
        self.inner.invoke_stream(request, sink).await
    }
}
