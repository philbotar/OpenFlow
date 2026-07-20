use async_trait::async_trait;
use engine::{
    emit_assistant_deltas_from_outcome, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamSink, ToolCall,
};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug)]
#[allow(dead_code, reason = "test helper variants for scripted AI responses")]
pub enum MockTurn {
    Completed {
        output: Value,
        assistant: Option<String>,
    },
    ToolCalls {
        calls: Vec<ToolCall>,
        assistant: Option<String>,
    },
    NeedsUserInput {
        message: String,
    },
    Error(AgentError),
}

#[allow(
    dead_code,
    reason = "test helper constructors for scripted AI responses"
)]
impl MockTurn {
    #[must_use]
    #[allow(
        clippy::missing_const_for_fn,
        reason = "serde_json::json! is not const"
    )]
    pub fn ok_json(output: Value) -> Self {
        Self::Completed {
            output,
            assistant: None,
        }
    }

    #[must_use]
    pub fn ok_summary(summary: &str) -> Self {
        Self::ok_json(serde_json::json!({ "summary": summary }))
    }

    #[must_use]
    pub fn transient(message: &str) -> Self {
        Self::Error(AgentError::Transient(message.to_string()))
    }

    #[must_use]
    pub fn permanent(message: &str) -> Self {
        Self::Error(AgentError::Permanent(message.to_string()))
    }

    #[must_use]
    pub fn failed(message: &str) -> Self {
        Self::Error(AgentError::Failed(message.to_string()))
    }

    /// Malformed final-output submit with a repairable candidate.
    #[must_use]
    pub fn malformed_submit_repairable(raw_arguments: &str, schema: &Value) -> Self {
        Self::Error(engine::malformed_submit_invalid_json(
            "mock",
            raw_arguments,
            "schema violation",
            Some(schema),
            Some("call_orig".into()),
            None,
            None,
        ))
    }

    /// Overseer completed turn with `repaired_arguments` accepted by the completion protocol.
    #[must_use]
    pub fn repaired_arguments(output: &Value) -> Self {
        Self::Completed {
            output: serde_json::json!({ "repaired_arguments": { "output": output } }),
            assistant: Some("overseer prose must clear".into()),
        }
    }

    #[must_use]
    pub fn tool_read(path: &str) -> Self {
        Self::ToolCalls {
            calls: vec![ToolCall {
                id: "call-read".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({ "path": path }),
            }],
            assistant: None,
        }
    }

    #[must_use]
    pub fn tool_bash(command: &str, timeout: u32) -> Self {
        Self::ToolCalls {
            calls: vec![ToolCall {
                id: "call-bash".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({ "command": command, "timeout": timeout }),
            }],
            assistant: None,
        }
    }
}

#[derive(Clone)]
pub struct MockAiStack {
    stack: Arc<Mutex<Vec<MockTurn>>>,
    requests: Arc<Mutex<Vec<AgentRequest>>>,
}

#[allow(
    dead_code,
    reason = "test harness for headless orchestration integration tests"
)]
impl MockAiStack {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            stack: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Script responses in invocation order: first entry is consumed on the first `invoke`.
    #[must_use]
    pub fn from_invocation_order(turns: impl IntoIterator<Item = MockTurn>) -> Self {
        let ordered: Vec<MockTurn> = turns.into_iter().collect();
        let mut stack = Vec::with_capacity(ordered.len());
        for turn in ordered.into_iter().rev() {
            stack.push(turn);
        }
        Self {
            stack: Arc::new(Mutex::new(stack)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push(&self, turn: MockTurn) {
        self.stack.lock().push(turn);
    }

    #[must_use]
    pub fn recorded_requests(&self) -> Vec<AgentRequest> {
        self.requests.lock().clone()
    }

    /// Requests whose node id ends with the engine repair suffix.
    #[must_use]
    pub fn recorded_repair_requests(&self) -> Vec<AgentRequest> {
        self.recorded_requests()
            .into_iter()
            .filter(|request| request.node_id.ends_with("__output_repair"))
            .collect()
    }

    fn pop_turn(&self) -> Option<MockTurn> {
        self.stack.lock().pop()
    }

    fn map_turn(turn: MockTurn) -> Result<AgentTurnOutcome, AgentError> {
        match turn {
            MockTurn::Completed { output, assistant } => {
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output,
                    raw_text: "{}".to_string(),
                    assistant_message: assistant,
                    usage: None,
                }))
            }
            MockTurn::ToolCalls { calls, assistant } => {
                Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: assistant,
                    tool_calls: calls,
                    reasoning: vec![],
                    usage: None,
                }))
            }
            MockTurn::NeedsUserInput { message } => {
                Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    raw_text: "{}".to_string(),
                    assistant_message: message,
                    reasoning: vec![],
                }))
            }
            MockTurn::Error(error) => Err(error),
        }
    }
}

#[async_trait]
impl AiPort for MockAiStack {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.requests.lock().push(request);
        self.pop_turn().map_or_else(
            || Err(AgentError::Failed("mock ai stack exhausted".to_string())),
            Self::map_turn,
        )
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let outcome = self.invoke(request).await?;
        emit_assistant_deltas_from_outcome(sink, &outcome);
        Ok(outcome)
    }
}
