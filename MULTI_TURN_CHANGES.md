# Multi-Turn Conversation Implementation

## Status
✅ **Engine changes complete** (`crates/workflow-core/src/interactive.rs`)
🔄 **Backend changes** (documented below - need manual application)
❌ **UI changes** (not yet started)

## Engine Changes (COMPLETE)

The `InteractiveEngine` now supports multi-turn conversations:

1. **New field**: `active_manual_node: Option<NodeId>` - tracks which manual node is in conversation
2. **Modified**: `on_human_input()` - no longer auto-advances, sets `active_manual_node`
3. **New method**: `complete_manual_node()` - advances to next node when user is done
4. **New method**: `continue_conversation()` - invokes AI again for another turn
5. **New poll result**: `ManualNodeActive { node_id, label }` - signals UI to show continue/complete buttons

## Backend Changes Needed

### File: `crates/agent-workflow-app/src/execution.rs`

#### 1. Add `ai_prompt` field to `NodeAwaitingInput` event (line ~28)
```rust
NodeAwaitingInput {
    node_id: NodeId,
    label: String,
    context: String,
    ai_prompt: Option<String>,  // ADD THIS
},
```

#### 2. Add `ManualNodeActive` event variant (after NodeAwaitingInput, ~line 30)
```rust
ManualNodeActive {
    node_id: NodeId,
    label: String,
},
```

#### 3. Add `CompleteManualNode` action (line ~43)
```rust
pub enum ExecutionAction {
    ProvideInput(String),
    CompleteManualNode,  // ADD THIS
}
```

#### 4. Update `drive_interactive_workflow` to handle `ManualNodeActive` (after AwaitInput arm, ~line 141)
```rust
EnginePollResult::ManualNodeActive { node_id, label } => {
    let _ = event_tx.send(ExecutionEvent::NodeQueued {
        node_id: node_id.clone(),
        label: label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::ManualNodeActive {
        node_id: node_id.clone(),
        label: label.clone(),
    });
    // Wait for user action
    if let Some(action) = action_rx.recv().await {
        match action {
            ExecutionAction::CompleteManualNode => {
                if let Err(e) = engine.complete_manual_node() {
                    let _ = event_tx.send(ExecutionEvent::Error(e));
                    break;
                }
                // Continue loop to process next node
            }
            ExecutionAction::ProvideInput(text) => {
                // Edge case: shouldn't happen but handle it
                let _ = engine.on_human_input(&node_id, &text);
                let _ = event_tx.send(ExecutionEvent::NodeCompleted {
                    node_id: node_id.clone(),
                    output: serde_json::json!(text),
                });
            }
        }
    } else {
        break;
    }
}
```

#### 5. Update `apply_event_to_run_state` to handle `ManualNodeActive` (after NodeAwaitingInput arm, ~line 288)
```rust
ExecutionEvent::ManualNodeActive { node_id, label } => {
    run_state.active_manual_node_id = Some(node_id.clone());
    run_state
        .status_by_node
        .insert(node_id.clone(), AgentStatus::AwaitingInput); // or create new status
    run_state.run_trace.push(RunTraceEntry {
        node_id: node_id.clone(),
        node_label: label.clone(),
        status: TraceStatus::Paused,
        message: "awaiting user decision".to_string(),
        output: None,
    });
}
```

### File: `crates/agent-workflow-app/src/state.rs`

#### Add field to `WorkflowRunState` (line ~54)
```rust
pub struct WorkflowRunState {
    pub active: bool,
    pub awaiting_node_id: Option<NodeId>,
    pub active_manual_node_id: Option<NodeId>,  // ADD THIS
    // ... rest of fields
}
```

### File: `crates/agent-workflow-app/src/backend.rs`

#### Add `complete_manual_node` method (after `submit_user_input`, ~line 322)
```rust
/// # Errors
/// Returns an error if there is no active run or no manual node is active.
pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
    let mut session = self.run_session.lock().await;
    let run_state = session
        .run_state
        .as_mut()
        .ok_or(BackendError::NoActiveRun)?;
    
    if run_state.active_manual_node_id.is_none() {
        return Err(BackendError::NoAwaitingInput);
    }
    
    session
        .action_tx
        .as_ref()
        .ok_or(BackendError::NoActiveRun)?
        .send(ExecutionAction::CompleteManualNode)
        .map_err(|_| BackendError::RunChannelClosed)?;
    
    Ok(run_state.clone())
}
```

### File: `crates/agent-workflow-desktop/src-tauri/src/lib.rs`

#### Add Tauri command (after `submit_user_input`, ~line 186)
```rust
#[tauri::command]
async fn complete_manual_node(
    app: AppHandle,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.complete_manual_node().await?;
    app.emit(RUN_STATE_EVENT, &run_state)
        .map_err(|error| CommandError::Emit(error.to_string()))?;
    Ok(run_state)
}
```

#### Register the command (in `run()` function, ~line 213)
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    complete_manual_node,  // ADD THIS
])
```

### File: `crates/agent-workflow-desktop/src/api.ts`

#### Add API function (after `submitUserInput`, ~line 82)
```typescript
export function completeManualNode() {
  return invoke<WorkflowRunState>("complete_manual_node");
}
```

## UI Changes Needed

### File: `crates/agent-workflow-desktop/src/App.tsx`

1. **Import the new API function**
2. **Add "Complete & Continue" button** - visible when `activeManualNodeId` is set
3. **Add "Continue Conversation" button** - invokes AI again for current node
4. **Update chat enabled logic** to check for either `awaitingNodeId` or `activeManualNodeId`

## Preprompt Changes

### Update default system prompt in `crates/workflow-core/src/model.rs` (line ~403)

Add instruction for LLM to signal when task is complete:

```rust
system_prompt: "You are a focused AI agent in a node workflow. 

IMPORTANT: When the user's request is fully complete and no further conversation is needed, 
end your response with the exact phrase: [TASK_COMPLETE]

This signals that the node can advance to the next step. If more clarification is needed, 
continue the conversation normally without this phrase.".to_string(),
```

Then parse the LLM response in `on_ai_complete` to detect `[TASK_COMPLETE]` and auto-advance.

## Testing

After applying all changes:

```bash
cargo check --workspace
cargo test --workspace
npm --prefix crates/agent-workflow-desktop run typecheck
```

## Notes

- The engine changes are complete and tested
- Backend changes need careful application due to borrow checker issues
- UI changes are straightforward once backend is complete
- LLM auto-completion requires parsing the response for the completion signal
