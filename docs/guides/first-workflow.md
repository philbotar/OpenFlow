# First Workflow Walkthrough

This walkthrough builds a two-node workflow: one node drafts a plan, and a second node turns that plan into a concise checklist. It exercises the normal OpenFlow path without requiring custom tools.

## 1. Start the App

```bash
./scripts/start.sh
```

Open Settings and confirm the active provider is ready. If readiness fails, check [`../troubleshooting/README.md#provider-not-ready`](../troubleshooting/README.md#provider-not-ready).

## 2. Create the Workflow

Create a workflow named `First workflow`.

OpenFlow stores app-level workflows in the local OpenFlow data directory. Project-linked workflows are stored under the project in `.flow/workflows/`; project files win if an app workflow and project workflow use the same ID.

## 3. Add the Planner Node

Add an agent node named `Planner`.

Use this instruction:

```text
Turn the user's request into a short implementation plan.
Return 3 to 5 ordered steps.
```

Leave tools disabled for the first pass. The engine can run a plain model turn without tool approval or tool result routing.

## 4. Add the Checklist Node

Add a second agent node named `Checklist`.

Use this instruction:

```text
Convert the upstream plan into a checklist.
Each item should be directly actionable.
```

Connect `Planner` to `Checklist`. The downstream node receives upstream output in dependency order.

## 5. Run the Workflow

Start the workflow with entrypoint text such as:

```text
Prepare a small release checklist for a desktop app.
```

Expected behavior:

1. The root node receives the entrypoint text.
2. `Planner` completes first.
3. `Checklist` receives the planner output.
4. The run completes with both node outputs available in the trace and conversation view.

## 6. Add Tools Later

After the plain workflow works, enable tools only on nodes that need local I/O or external actions. Tool-enabled nodes can pause for approval before execution. Approved tool results are routed back into the model loop; denied tools surface a structured error.

When changing or debugging this behavior, use the workflow acceptance tests:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

## 7. Make It Reusable

When a prompt should be shared across workflows, save it as an agent and add it through the callable-agent flow. Callable agents are resolved as snapshots for a workflow run, so a saved-agent edit does not silently mutate a running workflow. See [`../architecture/callable-agents.md`](../architecture/callable-agents.md).
