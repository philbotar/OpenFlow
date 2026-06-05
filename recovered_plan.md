# Workflow UI Upgrades Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add production-usable workflow editor UX: draggable nodes, explicit edge creation, UI-managed OpenAI key, label-based edge/readouts, agent status visibility, entrypoint input for root agents, plus targeted QoL and visual polish.

**Architecture:** Keep domain execution in `workflow-core`, keep editor/run state in `agent-workflow-app::state`, keep rendering + interaction wiring in `agent-workflow-app::ui`. Add a small pure math/presentation helper for canvas geometry so drag/edge behavior is testable without egui runtime. Route entrypoint input through runner API so root node requests receive it as structured JSON input.

**Tech Stack:** Rust 2021, `eframe`/`egui` 0.34, `tokio`, `serde_json`, `workflow-core` + `openai-client` crates.

---

## Scope Check

Single subsystem with one cross-cut seam (`workflow-core` runner input contract). Keep in one plan. No new service/process split needed.

## File Structure

- Modify: `crates/workflow-core/src/runner.rs`
- Modify: `crates/agent-workflow-app/src/state.rs`
- Modify: `crates/agent-workflow-app/src/ui.rs`
- Modify: `crates/agent-workflow-app/src/lib.rs`
- Create: `crates/agent-workflow-app/src/canvas_math.rs`
- Modify: `README.md`

Responsibilities:

- `runner.rs`: entrypoint-aware execution API, root-node input shaping tests.
- `state.rs`: editor state + commands (drag moves, edge dedupe, label mapping, API key/entrypoint fields, per-node status map).
- `canvas_math.rs`: pure geometry helpers (node clamp, connector anchors).
- `ui.rs`: toolbar/forms/canvas interactions + style polish + status chips + keyboard QoL.
- `lib.rs`: expose new module.
- `README.md`: document new UX features and updated run flow.

### Task 1: Add Entrypoint-Aware Runner API

**Files:**
- Modify: `crates/workflow-core/src/runner.rs`
- Test: `crates/workflow-core/src/runner.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn injects_entrypoint_into_root_node_input_only() {
    let mut workflow = Workflow::new("entrypoint");
    workflow.nodes = vec![node("idea"), node("plan")];
    workflow.edges = vec![Edge::new("idea", "plan")];

    let ai = RecordingAi::default();
    let requests_handle = ai.requests.clone();
    let runner = WorkflowRunner::new(ai);

    runner
        .run_with_entrypoint(&workflow, Some("Draft a launch plan"))
        .await
        .unwrap();

    let requests = requests_handle.lock().unwrap();
    let idea_req = requests.iter().find(|req| req.node_id == "idea").unwrap();
    let plan_req = requests.iter().find(|req| req.node_id == "plan").unwrap();

    assert_eq!(idea_req.input["entrypoint"]["text"], json!("Draft a launch plan"));
    assert!(plan_req.input.get("entrypoint").is_none());
}

#[tokio::test]
async fn run_without_entrypoint_preserves_existing_input_shape() {
    let mut workflow = Workflow::new("default");
    workflow.nodes = vec![node("idea")];

    let ai = RecordingAi::default();
    let requests_handle = ai.requests.clone();
    let runner = WorkflowRunner::new(ai);

    runner.run(&workflow).await.unwrap();

    let requests = requests_handle.lock().unwrap();
    assert_eq!(requests[0].input, json!({"upstream": []}));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p workflow-core injects_entrypoint_into_root_node_input_only -- --exact`
Expected: FAIL with `no method named 'run_with_entrypoint'`.

- [ ] **Step 3: Write minimal implementation**

```rust
pub async fn run(&self, workflow: &Workflow) -> Result<RunReport, RunError> {
    self.run_with_entrypoint(workflow, None).await
}

pub async fn run_with_entrypoint(
    &self,
    workflow: &Workflow,
    entrypoint_text: Option<&str>,
) -> Result<RunReport, RunError> {
    let layers = execution_layers(workflow)?;
    // unchanged setup...

    let responses: Vec<(NodeId, Result<AgentResponse, AgentError>)> = join_all(
        layer.iter().map(|node_id| {
            let request = AgentRequest {
                // unchanged fields...
                input: build_node_input(
                    node_id,
                    &upstream_by_node,
                    &outputs_by_node,
                    entrypoint_text,
                ),
                // unchanged fields...
            };
            async move { (node_id.clone(), self.ai.invoke(request).await) }
        }),
    )
    .await;

    // unchanged response handling...
}

fn build_node_input(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
    outputs_by_node: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
) -> Value {
    let upstream = upstream_by_node
        .get(node_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .filter_map(|id| {
            outputs_by_node.get(id).map(|output| {
                json!({
                    "node_id": id,
                    "output": output
                })
            })
        })
        .collect::<Vec<_>>();

    if upstream.is_empty() {
        if let Some(text) = entrypoint_text.filter(|text| !text.trim().is_empty()) {
            return json!({
                "entrypoint": { "text": text },
                "upstream": []
            });
        }
    }

    json!({ "upstream": upstream })
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p workflow-core runner::tests -- --nocapture`
Expected: PASS, includes new entrypoint tests.

- [ ] **Step 5: Commit**

```bash
git add crates/workflow-core/src/runner.rs
git commit -m "feat: support workflow entrypoint payload for root agents"
```

### Task 2: Extend AppState for Drag, Edge Labels, API Key, Entrypoint, Status

**Files:**
- Modify: `crates/agent-workflow-app/src/state.rs`
- Test: `crates/agent-workflow-app/src/state.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn rejects_duplicate_edges() {
    let mut state = AppState::new();
    let first = state.selected_node_id.clone().unwrap();
    let second = state.add_agent_node();

    state.select_node(first.clone());
    state.begin_link_from_selected();
    state.connect_link_to(second.clone());

    state.select_node(first);
    state.begin_link_from_selected();
    state.connect_link_to(second);

    assert_eq!(state.workflow.edges.len(), 1);
    assert_eq!(state.last_error.as_deref(), Some("edge already exists"));
}

#[test]
fn edge_rows_use_node_labels_not_ids() {
    let mut state = AppState::new();
    let first = state.selected_node_id.clone().unwrap();
    let second = state.add_agent_node();
    state.select_node(first.clone());
    state.begin_link_from_selected();
    state.connect_link_to(second.clone());

    let rows = state.edge_rows();

    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("Idea -> Agent 2"));
    assert!(!rows[0].contains(&first));
    assert!(!rows[0].contains(&second));
}

#[test]
fn moves_node_with_drag_delta_and_clamps_to_canvas() {
    let mut state = AppState::new();
    let node_id = state.selected_node_id.clone().unwrap();

    state.move_node_by_delta(&node_id, 20.0, 10.0, (640.0, 480.0), (220.0, 120.0));
    let moved = state.selected_node().unwrap().position.clone();
    assert!(moved.x >= 0.0);
    assert!(moved.y >= 0.0);

    state.move_node_by_delta(&node_id, -10_000.0, -10_000.0, (640.0, 480.0), (220.0, 120.0));
    let clamped = state.selected_node().unwrap().position.clone();
    assert_eq!(clamped.x, 0.0);
    assert_eq!(clamped.y, 0.0);
}

#[test]
fn ui_key_overrides_env_key_resolution() {
    let mut state = AppState::new();
    state.openai_api_key_input = "sk-ui-123".to_string();

    let key = state.resolve_api_key(Some("sk-env-456"));

    assert_eq!(key.as_deref(), Some("sk-ui-123"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-workflow-app state::tests::rejects_duplicate_edges -- --exact`
Expected: FAIL with missing methods/fields like `edge_rows`, `move_node_by_delta`, `resolve_api_key`.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Queued,
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub workflow: Workflow,
    pub selected_node_id: Option<NodeId>,
    pub link_from_node_id: Option<NodeId>,
    pub schema_editor_text: String,
    pub openai_api_key_input: String,
    pub entrypoint_text: String,
    pub status_by_node: std::collections::BTreeMap<NodeId, AgentStatus>,
    pub last_run: Option<RunReport>,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn resolve_api_key(&self, env_key: Option<&str>) -> Option<String> {
        if !self.openai_api_key_input.trim().is_empty() {
            return Some(self.openai_api_key_input.trim().to_string());
        }
        env_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    pub fn edge_rows(&self) -> Vec<String> {
        self.workflow
            .edges
            .iter()
            .map(|edge| {
                let from = self.node_label(&edge.from);
                let to = self.node_label(&edge.to);
                format!("{from} -> {to}")
            })
            .collect()
    }

    pub fn move_node_by_delta(
        &mut self,
        node_id: &str,
        dx: f32,
        dy: f32,
        canvas_size: (f32, f32),
        node_size: (f32, f32),
    ) {
        if let Some(node) = self.workflow.nodes.iter_mut().find(|node| node.id == node_id) {
            let max_x = (canvas_size.0 - node_size.0).max(0.0);
            let max_y = (canvas_size.1 - node_size.1).max(0.0);
            node.position.x = (node.position.x + dx).clamp(0.0, max_x);
            node.position.y = (node.position.y + dy).clamp(0.0, max_y);
        }
    }

    pub fn refresh_statuses_from_report(&mut self) {
        self.status_by_node.clear();
        for node in &self.workflow.nodes {
            self.status_by_node.insert(node.id.clone(), AgentStatus::Idle);
        }
        if let Some(report) = &self.last_run {
            for event in &report.events {
                let status = match event.kind {
                    RunEventKind::Queued => AgentStatus::Queued,
                    RunEventKind::Started => AgentStatus::Started,
                    RunEventKind::Completed => AgentStatus::Completed,
                    RunEventKind::Failed => AgentStatus::Failed,
                };
                self.status_by_node.insert(event.node_id.clone(), status);
            }
        }
    }

    fn node_label(&self, node_id: &str) -> String {
        self.workflow
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .map(|node| node.label.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn connect_link_to(&mut self, to_node_id: NodeId) {
        let Some(from_node_id) = self.link_from_node_id.take() else {
            return;
        };
        if from_node_id == to_node_id {
            self.last_error = Some("cannot connect a node to itself".to_string());
            return;
        }
        if self
            .workflow
            .edges
            .iter()
            .any(|edge| edge.from == from_node_id && edge.to == to_node_id)
        {
            self.last_error = Some("edge already exists".to_string());
            return;
        }
        self.workflow.edges.push(Edge::new(from_node_id, to_node_id));
        self.last_error = None;
    }
}
```

- [ ] **Step 4: Run state tests to verify pass**

Run: `cargo test -p agent-workflow-app state::tests -- --nocapture`
Expected: PASS, includes new drag/edge/key tests.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-workflow-app/src/state.rs
git commit -m "feat: extend app state for graph editing, key input, entrypoint, and statuses"
```

### Task 3: Add Pure Canvas Geometry Helper (Testable Drag/Edge Math)

**Files:**
- Create: `crates/agent-workflow-app/src/canvas_math.rs`
- Modify: `crates/agent-workflow-app/src/lib.rs`
- Test: `crates/agent-workflow-app/src/canvas_math.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn clamps_position_inside_canvas_bounds() {
    let pos = clamp_node_position((600.0, 430.0), (200.0, 120.0), (640.0, 480.0));
    assert_eq!(pos, (440.0, 360.0));
}

#[test]
fn edge_anchor_points_connect_right_to_left() {
    let (start, end) = edge_anchor_points((80.0, 120.0), (360.0, 120.0), (220.0, 120.0));
    assert_eq!(start, (300.0, 180.0));
    assert_eq!(end, (360.0, 180.0));
}
```

- [ ] **Step 2: Run tests to verify fail**

Run: `cargo test -p agent-workflow-app clamps_position_inside_canvas_bounds -- --exact`
Expected: FAIL with `cannot find function 'clamp_node_position'`.

- [ ] **Step 3: Write minimal implementation**

```rust
pub const NODE_WIDTH: f32 = 220.0;
pub const NODE_HEIGHT: f32 = 120.0;

pub fn clamp_node_position(
    pos: (f32, f32),
    node_size: (f32, f32),
    canvas_size: (f32, f32),
) -> (f32, f32) {
    let max_x = (canvas_size.0 - node_size.0).max(0.0);
    let max_y = (canvas_size.1 - node_size.1).max(0.0);
    (pos.0.clamp(0.0, max_x), pos.1.clamp(0.0, max_y))
}

pub fn edge_anchor_points(
    from_pos: (f32, f32),
    to_pos: (f32, f32),
    node_size: (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    (
        (from_pos.0 + node_size.0, from_pos.1 + node_size.1 * 0.5),
        (to_pos.0, to_pos.1 + node_size.1 * 0.5),
    )
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p agent-workflow-app canvas_math::tests -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-workflow-app/src/canvas_math.rs crates/agent-workflow-app/src/lib.rs
git commit -m "feat: add testable canvas geometry helpers"
```

### Task 4: Wire UI for Drag, Edge Builder, API Key, Entrypoint, Label-First Readouts, Agent Status

**Files:**
- Modify: `crates/agent-workflow-app/src/ui.rs`
- Modify: `crates/agent-workflow-app/src/state.rs`
- Test: `crates/agent-workflow-app/src/state.rs`

- [ ] **Step 1: Write the failing behavior test for state-backed edge composer path**

```rust
#[test]
fn edge_composer_connects_selected_source_and_target() {
    let mut state = AppState::new();
    let source = state.selected_node_id.clone().unwrap();
    let target = state.add_agent_node();

    state.link_from_node_id = Some(source.clone());
    state.connect_link_to(target.clone());

    assert_eq!(state.edge_rows(), vec!["Idea -> Agent 2".to_string()]);
}
```

- [ ] **Step 2: Run test to verify initial fail (before UI wiring update)**

Run: `cargo test -p agent-workflow-app edge_composer_connects_selected_source_and_target -- --exact`
Expected: FAIL until `edge_rows()` and dedupe behavior from Task 2 are in place.

- [ ] **Step 3: Implement UI wiring**

```rust
fn run_current_workflow(&mut self) {
    if let Err(error) = self.state.validate() {
        self.state.last_error = Some(error.to_string());
        return;
    }

    let api_key = match self
        .state
        .resolve_api_key(env::var("OPENAI_API_KEY").ok().as_deref())
    {
        Some(value) => value,
        None => {
            self.state.last_error = Some("OpenAI API key missing (UI field and OPENAI_API_KEY empty)".to_string());
            return;
        }
    };

    self.state.last_error = None;
    let client = OpenAiResponsesClient::new(api_key);
    let runner = WorkflowRunner::new(client);
    let entrypoint = self.state.entrypoint_text.trim().to_string();
    let entrypoint = if entrypoint.is_empty() { None } else { Some(entrypoint) };

    match self.runtime.block_on(runner.run_with_entrypoint(
        &self.state.workflow,
        entrypoint.as_deref(),
    )) {
        Ok(report) => {
            self.state.set_run_report(report);
            self.state.refresh_statuses_from_report();
        }
        Err(error) => self.state.last_error = Some(error.to_string()),
    }
}

// inside update toolbar:
ui.label("OpenAI key");
ui.add(
    egui::TextEdit::singleline(&mut self.state.openai_api_key_input)
        .password(true)
        .hint_text("sk-...")
        .desired_width(220.0),
);

// entrypoint block in inspector:
ui.label("Entrypoint input for root agents");
ui.add(
    egui::TextEdit::multiline(&mut self.state.entrypoint_text)
        .desired_rows(4)
        .hint_text("Describe what the first agent should do"),
);

// edge list now label-based:
for row in self.state.edge_rows() {
    ui.label(row);
}

// draggable nodes:
let node_id = egui::Id::new(("node", node.id.clone()));
let response = ui.interact(node_rect, node_id, egui::Sense::click_and_drag());
if response.dragged() {
    let delta = response.drag_delta();
    self.state.move_node_by_delta(
        &node.id,
        delta.x,
        delta.y,
        (rect.width(), rect.height()),
        (NODE_WIDTH, NODE_HEIGHT),
    );
}
if response.clicked() {
    self.state.select_node(node.id.clone());
}

// status chip on node card:
let status = self
    .state
    .status_by_node
    .get(&node.id)
    .copied()
    .unwrap_or(AgentStatus::Idle);
let (status_text, status_color) = match status {
    AgentStatus::Idle => ("IDLE", egui::Color32::from_gray(120)),
    AgentStatus::Queued => ("QUEUED", egui::Color32::from_rgb(120, 120, 220)),
    AgentStatus::Started => ("RUNNING", egui::Color32::from_rgb(76, 148, 255)),
    AgentStatus::Completed => ("DONE", egui::Color32::from_rgb(34, 176, 125)),
    AgentStatus::Failed => ("FAILED", egui::Color32::from_rgb(219, 72, 72)),
};
painter.text(
    node_rect.left_top() + egui::vec2(10.0, 10.0),
    egui::Align2::LEFT_TOP,
    status_text,
    egui::TextStyle::Small.resolve(ui.style()),
    status_color,
);
```

- [ ] **Step 4: Run tests and compile checks**

Run: `cargo test -p agent-workflow-app`
Expected: PASS.

Run: `cargo check -p agent-workflow-app`
Expected: PASS.

- [ ] **Step 5: Manual verification of requested UX**

Run: `cargo run -p agent-workflow-app`
Expected manual checks:
- Drag node on canvas -> node position persists during session.
- Add edge from UI controls -> edge appears as `Label A -> Label B`, no UUID.
- Enter OpenAI key in toolbar, leave env var unset -> run succeeds.
- Enter entrypoint text -> root node receives it (`Run trace` output context contains entrypoint input).
- Node cards show status chips (`IDLE/QUEUED/RUNNING/DONE/FAILED`).

- [ ] **Step 6: Commit**

```bash
git add crates/agent-workflow-app/src/ui.rs crates/agent-workflow-app/src/state.rs
git commit -m "feat: add draggable graph editor, key input, entrypoint, and node statuses"
```

### Task 5: AI-First Visual Polish + QoL Controls

**Files:**
- Modify: `crates/agent-workflow-app/src/ui.rs`
- Modify: `README.md`

- [ ] **Step 1: Write failing regression test for QoL guard (delete node removes dangling edges)**

```rust
#[test]
fn removing_selected_node_also_removes_incident_edges() {
    let mut state = AppState::new();
    let first = state.selected_node_id.clone().unwrap();
    let second = state.add_agent_node();

    state.select_node(first.clone());
    state.begin_link_from_selected();
    state.connect_link_to(second.clone());

    state.select_node(second);
    state.remove_selected_node();

    assert_eq!(state.workflow.nodes.len(), 1);
    assert!(state.workflow.edges.is_empty());
}
```

- [ ] **Step 2: Run test to verify fail**

Run: `cargo test -p agent-workflow-app removing_selected_node_also_removes_incident_edges -- --exact`
Expected: FAIL with missing `remove_selected_node`.

- [ ] **Step 3: Implement QoL state + UI actions**

```rust
// state.rs
pub fn remove_selected_node(&mut self) {
    let Some(selected) = self.selected_node_id.clone() else { return; };
    self.workflow.nodes.retain(|node| node.id != selected);
    self.workflow
        .edges
        .retain(|edge| edge.from != selected && edge.to != selected);
    self.selected_node_id = self.workflow.nodes.first().map(|node| node.id.clone());
    self.refresh_schema_editor();
}

// ui.rs toolbar buttons and shortcuts
if ui.button("Delete node").clicked() {
    self.state.remove_selected_node();
}
if ui.button("Clear run").clicked() {
    self.state.last_run = None;
    self.state.refresh_statuses_from_report();
}

let run_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
if run_shortcut {
    self.run_current_workflow();
}
let save_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));
if save_shortcut {
    self.save_workflow();
}

// visual polish direction (AI-first look)
ctx.set_visuals(egui::Visuals {
    window_fill: egui::Color32::from_rgb(10, 14, 20),
    panel_fill: egui::Color32::from_rgb(15, 20, 29),
    extreme_bg_color: egui::Color32::from_rgb(7, 10, 15),
    ..egui::Visuals::dark()
});
```

- [ ] **Step 4: Update README feature list**

```markdown
- Drag nodes on canvas; connect agents with explicit edge builder controls.
- Provide OpenAI API key in-app (secure input) or fall back to `OPENAI_API_KEY`.
- Provide entrypoint input text routed to root agents.
- View per-agent execution status chips and run trace.
- Use keyboard QoL: `Cmd/Ctrl+Enter` run, `Cmd/Ctrl+S` save, delete selected node.
```

- [ ] **Step 5: Run full verification sweep**

Run: `cargo fmt --all --check`
Expected: PASS.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-workflow-app/src/state.rs crates/agent-workflow-app/src/ui.rs README.md
git commit -m "feat: polish workflow UI and add quality-of-life controls"
```

## Ownership

- `crates/workflow-core/src/runner.rs`: owns entrypoint payload propagation for root-node agent requests.
- `crates/agent-workflow-app/src/state.rs`: owns graph-edit commands, API key resolution, entrypoint storage, status derivation, and QoL state actions.
- `crates/agent-workflow-app/src/canvas_math.rs`: owns deterministic geometry math for drag clamps and edge anchors.
- `crates/agent-workflow-app/src/ui.rs`: owns visual composition, edge creation controls, drag interactions, status chips, style system, shortcuts.
- `crates/agent-workflow-app/src/lib.rs`: owns module exposure for new canvas helper.
- `README.md`: owns user-facing feature + run guidance for new UI capabilities.

## Self-Review

- Spec coverage check: all requested items mapped.
  - Drag nodes: Task 2 + Task 4.
  - Add edges: Task 2 + Task 4.
  - Add OpenAI key in UI: Task 2 + Task 4.
  - Hide UUID in edges: Task 2 + Task 4.
  - Beautify AI-first look: Task 5.
  - Show AGENT status: Task 2 + Task 4.
  - Show entrypoint input to first agent: Task 1 + Task 4.
  - Additional QoL: Task 5 (delete node cascade, clear run, shortcuts).
- Placeholder scan: no TBD/TODO placeholders; each task has concrete code and commands.
- Type consistency check: shared names are consistent (`run_with_entrypoint`, `entrypoint_text`, `AgentStatus`, `edge_rows`, `move_node_by_delta`, `remove_selected_node`).
