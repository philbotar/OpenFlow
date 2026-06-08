# Domain

`crates/domain`

## What it does

Pure workflow engine: graph model, validation, template definitions, tool policy types, and two execution modes.

| Module | Glossary terms | Role |
| --- | --- | --- |
| `graph/` | Workflow, Node, Edge, CallableAgent, execution layers | DAG model, saved-agent snapshots, `validate_workflow` |
| `template/` | Template, LockedField | Reusable node presets; `TemplateStore` seam |
| `execution/` | WorkflowRunner, InteractiveEngine, RunTelemetry, subagent_runtime | Batch vs step-through runs; subagent turn machine; shared `node_invocation` |
| `conversation/` | ChatMessage, AgentTranscriptItem | Transcript and chat DTOs |
| `tools/` | NodeToolConfig, ApprovalMode | Tool catalog selection and approval policy |
| `ports/` | AiPort, inbound ports | Outbound AI seam and human/tool input contracts |

## Why it is structured this way

File names match [`docs/glossary.md`](../../glossary.md) so navigation and docs use the same words.

`WorkflowRunner` is the non-interactive path (one AI turn per node, no tools or pauses). `InteractiveEngine` is what orchestration drives for the desktop app.

`FileTemplateStore` lives in orchestration — domain owns the `Template` type and `TemplateStore` trait only.
