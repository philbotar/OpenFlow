# Run persistence

Orchestration persists each interactive workflow attempt as a run record with append-only checkpoints. It stores snapshots, rebuilds host resources on resume, and projects replay state for the UI. The engine owns execution behavior.

## Decisions

| ID | Decision |
| --- | --- |
| R1 | A run id identifies one attempt. Resume after restart keeps the same run id. |
| R2 | Project-assigned workflows store runs under `{project}/.flow/runs/`; app-only workflows under `{data_local}/openflow/runs/`. |
| R3 | Each checkpoint stores `InteractiveEngineCheckpoint`, `WorkflowRunState`, and the artifact summaries already in the projection. |
| R4 | A checkpoint is appended after human-input, tool-approval, and retry pauses, user stops, successful completion, and terminal failures. |
| R5 | Read-only replay loads the latest checkpoint projection. It never starts `drive.rs` or calls a provider. |
| R6 | Durable resume loads the latest checkpoint and recorded workflow snapshot, refreshes only runtime-editable settings (model, reasoning, approval mode, speed) from the current inspector state, updates the recorded hash, validates the checkpoint, rebuilds host resources, then starts `drive.rs` with `resume_checkpoint`. Prompts, graph structure, prior outputs, and transcripts stay pinned to the run. |
| R7 | Forking and export are out of scope. |

## Replay versus resume

- **Replay** returns `WorkflowRunState` with `active = false` and clears pending approvals, so old approval buttons cannot execute.
- **Resume** executes from the checkpoint engine state, keeps the original run id, writes future checkpoints to the same run directory, and reuses the same artifact directory.
