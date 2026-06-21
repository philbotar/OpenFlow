//! Shared orchestration integration-test helpers (mock AI stack, harness, workflows).

#![allow(
    dead_code,
    unused_imports,
    reason = "integration test support re-exports"
)]

mod mock_ai_stack;
mod run_harness;
mod workflows;

pub use mock_ai_stack::{MockAiStack, MockTurn};
pub use run_harness::{run_headless_script, spawn_interactive_script, HeadlessRunOpts};
pub use workflows::{agent_node, branch_join_workflow, linear_workflow, single_agent_workflow};
