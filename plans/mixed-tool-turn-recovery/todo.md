# Mixed Tool Turn Recovery

**Goal:** Prevent custom OpenAI-compatible nodes from failing permanently when a model mixes control and executable tool calls in one response.

**Architecture:** The provider adapter keeps rejecting unsafe mixed batches, but reports a typed engine error. The engine adds a bounded transcript correction and re-invokes the same node without executing any calls from the rejected batch. Runtime guidance and focused tests document the required control/work sequencing and artifact-backed submission.

**Tech Stack:** Rust workspace, `engine` execution semantics, `providers` OpenAI-compatible mapping, orchestration acceptance tests.

---

- [x] 1. Add typed mixed-turn error classification and provider mapping coverage
- [x] 2. Retry rejected mixed turns with explicit correction and persisted recovery state
- [x] 3. Strengthen runtime guidance and verify the end-to-end protocol
