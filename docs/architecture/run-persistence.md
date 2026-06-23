# Run Persistence

Durable run persistence stores interactive workflow attempts as append-only run records. Engine behavior remains in `engine`; orchestration stores snapshots, resumes host resources, and projects replay state for the UI.

## Decisions

| ID | Decision |
| --- | --- |
| R1 | A run id identifies one attempt. Resume after restart keeps the same run id. |
| R2 | Project-assigned workflows store runs under `{project}/.flow/runs/`; app-only workflows store under `{data_local}/openflow/runs/`. |
| R3 | Each checkpoint stores `InteractiveEngineCheckpoint`, `WorkflowRunState`, and the artifact summaries already present in the projection. |
| R4 | Checkpoints are appended after human-input pauses, tool-approval pauses, retry pauses, user stops, successful completion, and terminal failures. |
| R5 | Read-only replay loads the latest checkpoint projection and never starts `drive.rs` or calls a provider. |
| R6 | Durable resume loads the latest checkpoint, validates it against the current workflow and workflow hash, rebuilds host resources, then starts `drive.rs` with `resume_checkpoint`. |
| R7 | Forking and export are intentionally out of this slice. |

## Replay Versus Resume

Replay is a UI inspection mode. It returns `WorkflowRunState` with `active = false` and clears pending approvals so old approval buttons cannot execute.

Resume is an execution mode. It uses the checkpoint engine state, keeps the original run id, writes future checkpoints to the same run directory, and reuses the same artifact directory.
