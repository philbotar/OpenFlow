import { onCleanup, onMount, createEffect, createMemo, createSignal, For, Show } from "solid-js";
import PanelLeftClose from "lucide-solid/icons/panel-left-close";
import PanelLeftOpen from "lucide-solid/icons/panel-left-open";
import {
  bootstrapApp,
  clearRunTrace,
  createAgentNode,
  createWorkflow,
  resolveProviderReadiness,
  saveSettings,
  saveWorkflows,
  startRun,
  submitUserInput,
  validateWorkflow,
  listenToRunState,
} from "./api";
import type {
  AiProviderKind,
  AppSettings,
  Node as WorkflowNode,
  NodeId,
  ProviderTransport,
  RunTraceEntry,
  Workflow,
  WorkflowRunState,
} from "./types";
import {
  NODE_HEIGHT,
  NODE_WIDTH,
  activeProfile,
  cloneSettings,
  cloneWorkflow,
  createIdleRunState,
  nodeOutput,
  prettyJson,
  removeSelectedNode,
  replaceWorkflow,
  selectedNode,
  statusForNode,
} from "./workflow";

type Banner = { kind: "error" | "success" | "info"; text: string } | null;
type BottomTab = "chat" | "trace";
type DragState = {
  nodeId: NodeId;
  pointerId: number;
  startX: number;
  startY: number;
  originX: number;
  originY: number;
} | null;

const EMPTY_SETTINGS: AppSettings = {
  active_provider: "open_ai",
  openai: {
    display_name: "ChatGPT / OpenAI",
    base_url: "https://api.openai.com",
    transport: "responses",
    responses_path: "v1/responses",
    chat_completions_path: "v1/chat/completions",
    known_models: ["gpt-4o", "gpt-4o-mini", "gpt-4.5", "o3"],
  },
  openai_compatible: {
    display_name: "OpenAI-compatible API",
    base_url: "http://localhost:11434",
    transport: "chat_completions",
    responses_path: "v1/responses",
    chat_completions_path: "v1/chat/completions",
    known_models: ["llama3.1", "qwen2.5", "mistral"],
  },
};

function App() {
  const [workflows, setWorkflows] = createSignal<Workflow[]>([]);
  const [activeWorkflowId, setActiveWorkflowId] = createSignal<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = createSignal<NodeId | null>(null);
  const [linkFromNodeId, setLinkFromNodeId] = createSignal<NodeId | null>(null);
  const [showSettings, setShowSettings] = createSignal(false);
  const [settings, setSettings] = createSignal<AppSettings>(cloneSettings(EMPTY_SETTINGS));
  const [transientApiKey, setTransientApiKey] = createSignal("");
  const [entrypointText, setEntrypointText] = createSignal("");
  const [runState, setRunState] = createSignal<WorkflowRunState | null>(null);
  const [readiness, setReadiness] = createSignal<{ ready: boolean; provider: string; message: string; envVar: string } | null>(null);
  const [banner, setBanner] = createSignal<Banner>(null);
  const [sidebarOpen, setSidebarOpen] = createSignal(true);
  const [bottomTab, setBottomTab] = createSignal<BottomTab>("chat");
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [schemaText, setSchemaText] = createSignal("");
  const [chatInput, setChatInput] = createSignal("");
  const [dragState, setDragState] = createSignal<DragState>(null);
  const [newModelInputByProvider, setNewModelInputByProvider] = createSignal<Record<AiProviderKind, string>>({
    open_ai: "",
    open_ai_compatible: "",
  });
  const [editingWorkflowId, setEditingWorkflowId] = createSignal<string | null>(null);
  const [workflowNameDraft, setWorkflowNameDraft] = createSignal("");

  let canvasRef: HTMLDivElement | undefined;

  const activeWorkflow = createMemo(() =>
    workflows().find((workflow) => workflow.id === activeWorkflowId()),
  );
  const currentNode = createMemo(() =>
    selectedNode(activeWorkflow(), selectedNodeId()),
  );
  const activeProfileMemo = createMemo(() => activeProfile(settings()));
  const selectedTrace = createMemo<RunTraceEntry | null>(() => {
    const index = selectedTraceIndex();
    if (index === null) {
      return null;
    }
    return runState()?.runTrace[index] ?? null;
  });
  const currentNodeOutput = createMemo(() =>
    nodeOutput(runState(), selectedNodeId()),
  );
  const chatMessages = createMemo(() => {
    const nodeId = selectedNodeId();
    if (!nodeId) {
      return [];
    }
    return runState()?.chatLogs[nodeId] ?? [];
  });
  const chatEnabledMemo = createMemo(() =>
    runState()?.active === true &&
    runState()?.awaitingNodeId === selectedNodeId() &&
    (readiness()?.ready ?? false),
  );
  const canSendChatMemo = createMemo(() =>
    chatEnabledMemo() && chatInput().trim() !== "",
  );

  const setError = (text: string) => setBanner({ kind: "error", text });
  const setSuccess = (text: string) => setBanner({ kind: "success", text });
  const setInfo = (text: string) => setBanner({ kind: "info", text });

  const refreshReadiness = async (
    nextSettings = settings(),
    nextTransientApiKey = transientApiKey(),
  ) => {
    try {
      setReadiness(await resolveProviderReadiness(nextSettings, nextTransientApiKey));
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const initializeWorkspace = async (initialWorkflows: Workflow[], initialSettings: AppSettings, initialRunState: WorkflowRunState | null) => {
    let nextWorkflows = initialWorkflows;
    if (nextWorkflows.length === 0) {
      nextWorkflows = [await createWorkflow("Workflow 1")];
    }
    const firstWorkflow = nextWorkflows[0];
    setWorkflows(nextWorkflows);
    setActiveWorkflowId(firstWorkflow.id);
    setSelectedNodeId(firstWorkflow.nodes[0]?.id ?? null);
    setRunState(initialRunState ?? createIdleRunState(firstWorkflow));
    setSettings(cloneSettings(initialSettings));
    setBanner(null);
    await refreshReadiness(initialSettings, transientApiKey());
  };

  onMount(async () => {
    const unlisten = await listenToRunState((nextRunState) => {
      setRunState(nextRunState);
      if (nextRunState.awaitingNodeId) {
        setSelectedNodeId(nextRunState.awaitingNodeId);
        setBottomTab("chat");
      }
      if (nextRunState.lastError) {
        setError(nextRunState.lastError);
      }
    });

    onCleanup(() => {
      void unlisten();
    });

    try {
      const data = await bootstrapApp();
      await initializeWorkspace(data.workflows, data.settings, data.runState);
    } catch (error) {
      setError(normalizeError(error));
    }
  });

  createEffect(() => {
    const node = currentNode();
    setSchemaText(node ? prettyJson(node.agent.output_schema) : "");
  });

  const updateSettings = async (mutator: (draft: AppSettings) => void) => {
    const next = cloneSettings(settings());
    mutator(next);
    setSettings(next);
    await refreshReadiness(next, transientApiKey());
  };

  const updateActiveWorkflow = (mutator: (draft: Workflow) => void) => {
    const workflow = activeWorkflow();
    if (!workflow) {
      return;
    }
    const next = cloneWorkflow(workflow);
    mutator(next);
    setWorkflows(replaceWorkflow(workflows(), next));
  };

  const applySchemaEditor = () => {
    const nodeId = selectedNodeId();
    const workflow = activeWorkflow();
    if (!nodeId || !workflow) {
      return true;
    }
    try {
      const parsed = JSON.parse(schemaText());
      updateActiveWorkflow((draft) => {
        const node = draft.nodes.find((item) => item.id === nodeId);
        if (node) {
          node.agent.output_schema = parsed;
        }
      });
      setBanner(null);
      return true;
    } catch (error) {
      setError(`output schema JSON invalid: ${normalizeError(error)}`);
      return false;
    }
  };

  const persistAll = async (successText = "Saved") => {
    if (!applySchemaEditor()) {
      return false;
    }
    try {
      await saveWorkflows(workflows());
      await saveSettings(settings());
      setSuccess(successText);
      return true;
    } catch (error) {
      setError(normalizeError(error));
      return false;
    }
  };

  const handleSwitchWorkflow = (workflowId: string) => {
    if (!applySchemaEditor()) {
      return;
    }
    const workflow = workflows().find((item) => item.id === workflowId);
    if (!workflow) {
      return;
    }
    setActiveWorkflowId(workflow.id);
    setSelectedNodeId(workflow.nodes[0]?.id ?? null);
    setShowSettings(false);
    setLinkFromNodeId(null);
    setSelectedTraceIndex(null);
  };

  const handleCreateWorkflow = async () => {
    try {
      const workflow = await createWorkflow(`Workflow ${workflows().length + 1}`);
      setWorkflows([...workflows(), workflow]);
      setActiveWorkflowId(workflow.id);
      setSelectedNodeId(workflow.nodes[0]?.id ?? null);
      setShowSettings(false);
      setSuccess("Created workflow");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleAddNode = async () => {
    const workflow = activeWorkflow();
    if (!workflow) {
      return;
    }
    try {
      const node = await createAgentNode(
        workflow.nodes.length,
        96 + workflow.nodes.length * 32,
        96 + workflow.nodes.length * 20,
      );
      updateActiveWorkflow((draft) => {
        draft.nodes.push(node);
      });
      setSelectedNodeId(node.id);
      setSuccess("Added node");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleDeleteSelectedNode = () => {
    const workflow = activeWorkflow();
    if (!workflow) {
      return;
    }
    const next = removeSelectedNode(workflow, selectedNodeId());
    setWorkflows(replaceWorkflow(workflows(), next));
    setSelectedNodeId(next.nodes[0]?.id ?? null);
    setLinkFromNodeId(null);
  };

  const handleValidate = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) {
      return;
    }
    try {
      const summary = await validateWorkflow(activeWorkflow()!);
      setSuccess(`Valid DAG · ${summary.layerCount} layer${summary.layerCount === 1 ? "" : "s"}`);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleRun = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) {
      return;
    }
    try {
      const nextRunState = await startRun(
        activeWorkflow()!,
        entrypointText(),
        settings(),
        transientApiKey(),
      );
      setRunState(nextRunState);
      setSelectedTraceIndex(null);
      setBottomTab("chat");
      setBanner(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    try {
      const nextRunState = await clearRunTrace();
      if (nextRunState) {
        setRunState(nextRunState);
      }
      setSelectedTraceIndex(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSubmitChat = async () => {
    const nodeId = selectedNodeId();
    if (!nodeId || !canSendChatMemo()) {
      return;
    }
    try {
      const nextRunState = await submitUserInput(nodeId, chatInput().trim());
      setRunState(nextRunState);
      setChatInput("");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleBeginLink = () => {
    if (selectedNodeId()) {
      setLinkFromNodeId(selectedNodeId());
    }
  };

  const handleConnectOrSelectNode = (nodeId: NodeId) => {
    const linkFrom = linkFromNodeId();
    if (linkFrom && linkFrom !== nodeId) {
      updateActiveWorkflow((draft) => {
        const duplicate = draft.edges.some(
          (edge) => edge.from === linkFrom && edge.to === nodeId,
        );
        if (!duplicate) {
          draft.edges.push({
            id: crypto.randomUUID(),
            from: linkFrom,
            to: nodeId,
          });
        }
      });
      setLinkFromNodeId(null);
      setSelectedNodeId(nodeId);
      return;
    }
    setSelectedNodeId(nodeId);
  };

  const handleWorkflowNameCommit = () => {
    const workflowId = editingWorkflowId();
    if (!workflowId) {
      return;
    }
    const nextName = workflowNameDraft().trim();
    if (nextName !== "") {
      setWorkflows(
        workflows().map((workflow) =>
          workflow.id === workflowId ? { ...workflow, name: nextName } : workflow,
        ),
      );
    }
    setEditingWorkflowId(null);
    setWorkflowNameDraft("");
  };

  const handleAddKnownModel = () => {
    const provider = settings().active_provider;
    const nextName = newModelInputByProvider()[provider].trim();
    if (nextName === "") {
      return;
    }
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      if (!profile.known_models.includes(nextName)) {
        profile.known_models = [...profile.known_models, nextName];
      }
    });
    setNewModelInputByProvider({
      ...newModelInputByProvider(),
      [provider]: "",
    });
  };

  const handleRemoveKnownModel = (model: string) => {
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      profile.known_models = profile.known_models.filter((item) => item !== model);
    });
  };

  const handleCanvasPointerMove = (event: PointerEvent) => {
    const currentDrag = dragState();
    const workflow = activeWorkflow();
    if (!currentDrag || !workflow || !canvasRef) {
      return;
    }
    if (event.pointerId !== currentDrag.pointerId) {
      return;
    }
    const rect = canvasRef.getBoundingClientRect();
    const maxX = Math.max(rect.width - NODE_WIDTH, 0);
    const maxY = Math.max(rect.height - NODE_HEIGHT, 0);
    const nextX = clamp(currentDrag.originX + event.clientX - currentDrag.startX, 0, maxX);
    const nextY = clamp(currentDrag.originY + event.clientY - currentDrag.startY, 0, maxY);
    updateActiveWorkflow((draft) => {
      const node = draft.nodes.find((item) => item.id === currentDrag.nodeId);
      if (node) {
        node.position.x = nextX;
        node.position.y = nextY;
      }
    });
  };

  const handleCanvasPointerUp = (event: PointerEvent) => {
    const currentDrag = dragState();
    if (!currentDrag || event.pointerId !== currentDrag.pointerId) {
      return;
    }
    setDragState(null);
  };

  onMount(() => {
    window.addEventListener("pointermove", handleCanvasPointerMove);
    window.addEventListener("pointerup", handleCanvasPointerUp);
    window.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    window.removeEventListener("pointermove", handleCanvasPointerMove);
    window.removeEventListener("pointerup", handleCanvasPointerUp);
    window.removeEventListener("keydown", handleKeyDown);
  });

  function handleKeyDown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void persistAll();
      return;
    }
    if (command && event.key === "Enter") {
      event.preventDefault();
      void handleRun();
      return;
    }
    if ((event.key === "Delete" || event.key === "Backspace") && !isTextInputTarget(event.target)) {
      event.preventDefault();
      handleDeleteSelectedNode();
    }
  }

  return (
    <div class="app-shell">
      <aside class="sidebar" classList={{ "sidebar-hidden": !sidebarOpen() }}>
        <div class="sidebar-header">
          <div style="display: flex; align-items: center; gap: 8px;">
            <button
              type="button"
              class="sidebar-hide-button"
              onClick={() => setSidebarOpen(false)}
              title="Hide sidebar"
              aria-label="Hide sidebar"
            >
              <PanelLeftClose aria-hidden="true" absoluteStrokeWidth strokeWidth={2} />
            </button>
            <div>
              <div class="eyebrow">Workspace</div>
              <h1>Workflows</h1>
            </div>
          </div>
          <button class="secondary-button" onClick={() => void handleCreateWorkflow()}>
            New
          </button>
        </div>
        <div class="sidebar-list">
          <For each={workflows()}>
            {(workflow) => {
              const active = () => workflow.id === activeWorkflowId();
              const editing = () => workflow.id === editingWorkflowId();
              return (
                <div class="workflow-row" classList={{ active: active() }}>
                  <button
                    class="workflow-row-main"
                    onClick={() => handleSwitchWorkflow(workflow.id)}
                  >
                    <div class="workflow-row-copy">
                      <Show
                        when={!editing()}
                        fallback={
                          <input
                            value={workflowNameDraft()}
                            onInput={(event) => setWorkflowNameDraft(event.currentTarget.value)}
                            onBlur={handleWorkflowNameCommit}
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                handleWorkflowNameCommit();
                              }
                            }}
                            class="workflow-row-input"
                            autofocus
                          />
                        }
                      >
                        <span>{workflow.name}</span>
                      </Show>
                      <small>{workflow.nodes.length} nodes</small>
                    </div>
                  </button>
                  <button
                    class="icon-button"
                    onClick={() => {
                      setEditingWorkflowId(workflow.id);
                      setWorkflowNameDraft(workflow.name);
                    }}
                    title="Rename workflow"
                  >
                    ✎
                  </button>
                </div>
              );
            }}
          </For>
        </div>
        <div class="sidebar-footer">
          <button class="secondary-button stretch" onClick={() => setShowSettings(!showSettings())}>
            {showSettings() ? "Back to editor" : "Settings"}
          </button>
        </div>
      </aside>

      <main class="main-shell">
        <header class="topbar">
          <div class="topbar-leading">
            <Show when={!sidebarOpen()}>
              <button
                type="button"
                class="topbar-icon-button"
                onClick={() => setSidebarOpen(true)}
                title="Show sidebar"
                aria-label="Show sidebar"
              >
                <PanelLeftOpen aria-hidden="true" absoluteStrokeWidth strokeWidth={2} />
              </button>
            </Show>
            <div class="topbar-copy">
              <div class="eyebrow">Step-through Agentic Workflow</div>
              <h2>{activeWorkflow()?.name ?? "Loading…"}</h2>
            </div>
          </div>
          <div class="topbar-actions">
            <div class="readiness-chip" classList={{ ready: readiness()?.ready }}>
              <span class="status-dot" />
              <span>{readiness()?.message ?? "Checking provider"}</span>
            </div>
            <button class="secondary-button" onClick={() => void persistAll()}>
              Save
            </button>
            <button class="secondary-button" onClick={() => void handleValidate()}>
              Validate
            </button>
            <button class="primary-button" onClick={() => void handleRun()}>
              Run
            </button>
          </div>
        </header>

        <Show
          when={!showSettings()}
          fallback={
            <section class="settings-screen">
              <div class="settings-panel">
                <div class="settings-section">
                  <div>
                    <div class="eyebrow">Authentication</div>
                    <h3>Transient API key</h3>
                    <p>This key stays in memory only. Environment variables still act as fallback.</p>
                  </div>
                  <input
                    type="password"
                    value={transientApiKey()}
                    onInput={(event) => {
                      const value = event.currentTarget.value;
                      setTransientApiKey(value);
                      void refreshReadiness(settings(), value);
                    }}
                    placeholder={readiness()?.envVar ?? "OPENAI_API_KEY"}
                    class="text-input"
                  />
                </div>

                <div class="settings-section">
                  <div>
                    <div class="eyebrow">Provider</div>
                    <h3>Execution transport</h3>
                  </div>
                  <div class="segmented-control">
                    <button
                      classList={{ active: settings().active_provider === "open_ai" }}
                      onClick={() => void updateSettings((draft) => {
                        draft.active_provider = "open_ai";
                      })}
                    >
                      OpenAI
                    </button>
                    <button
                      classList={{ active: settings().active_provider === "open_ai_compatible" }}
                      onClick={() => void updateSettings((draft) => {
                        draft.active_provider = "open_ai_compatible";
                      })}
                    >
                      Compatible
                    </button>
                  </div>
                  <div class="field-grid">
                    <label>
                      <span>Base URL</span>
                      <input
                        class="text-input"
                        value={activeProfileMemo().base_url}
                        onInput={(event) =>
                          void updateSettings((draft) => {
                            activeProfile(draft).base_url = event.currentTarget.value;
                          })
                        }
                      />
                    </label>
                    <label>
                      <span>Transport</span>
                      <select
                        class="text-input"
                        value={activeProfileMemo().transport}
                        onChange={(event) =>
                          void updateSettings((draft) => {
                            activeProfile(draft).transport = event.currentTarget.value as ProviderTransport;
                          })
                        }
                      >
                        <option value="responses">Responses API</option>
                        <option value="chat_completions">Chat Completions API</option>
                      </select>
                    </label>
                    <label>
                      <span>Responses path</span>
                      <input
                        class="text-input"
                        value={activeProfileMemo().responses_path}
                        onInput={(event) =>
                          void updateSettings((draft) => {
                            activeProfile(draft).responses_path = event.currentTarget.value;
                          })
                        }
                      />
                    </label>
                    <label>
                      <span>Chat completions path</span>
                      <input
                        class="text-input"
                        value={activeProfileMemo().chat_completions_path}
                        onInput={(event) =>
                          void updateSettings((draft) => {
                            activeProfile(draft).chat_completions_path = event.currentTarget.value;
                          })
                        }
                      />
                    </label>
                  </div>
                </div>

                <div class="settings-section">
                  <div>
                    <div class="eyebrow">Models</div>
                    <h3>Known models for the active provider</h3>
                  </div>
                  <div class="chip-list">
                    <For each={activeProfileMemo().known_models}>
                      {(model) => (
                        <button class="model-chip" onClick={() => handleRemoveKnownModel(model)}>
                          {model}
                          <span>×</span>
                        </button>
                      )}
                    </For>
                  </div>
                  <div class="inline-form">
                    <input
                      class="text-input"
                      placeholder="Add model"
                      value={newModelInputByProvider()[settings().active_provider]}
                      onInput={(event) =>
                        setNewModelInputByProvider({
                          ...newModelInputByProvider(),
                          [settings().active_provider]: event.currentTarget.value,
                        })
                      }
                    />
                    <button class="secondary-button" onClick={handleAddKnownModel}>
                      Add model
                    </button>
                  </div>
                  <button class="primary-button" onClick={() => void persistAll("Saved settings")}>Save settings</button>
                </div>
              </div>
            </section>
          }
        >
          <Show when={banner()}>
            {(currentBanner) => (
              <div class="banner" classList={{ error: currentBanner().kind === "error", success: currentBanner().kind === "success" }}>
                {currentBanner().text}
              </div>
            )}
          </Show>

          <div class="workspace-grid">
            <section class="canvas-panel">
              <div class="canvas-toolbar">
                <div class="toolbar-group">
                  <button class="secondary-button" onClick={() => void handleAddNode()}>
                    Add node
                  </button>
                  <button class="secondary-button" onClick={handleBeginLink} disabled={!selectedNodeId()}>
                    {linkFromNodeId() ? `Linking from ${linkFromNodeId()}` : "Link selected"}
                  </button>
                </div>
                <label class="entrypoint-field">
                  <span>Entrypoint</span>
                  <input
                    class="text-input"
                    value={entrypointText()}
                    onInput={(event) => setEntrypointText(event.currentTarget.value)}
                    placeholder="Root nodes receive entrypoint.text"
                  />
                </label>
              </div>
              <div class="canvas-board" ref={canvasRef}>
                <Show when={activeWorkflow()}>
                  {(workflow) => (
                    <>
                      <svg class="edge-layer" viewBox={`0 0 ${Math.max(canvasRef?.clientWidth ?? 1, 1)} ${Math.max(canvasRef?.clientHeight ?? 1, 1)}`} preserveAspectRatio="none">
                        <defs>
                          <marker id="edge-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
                            <path d="M0 0 L8 4 L0 8 Z" fill="currentColor" />
                          </marker>
                        </defs>
                        <For each={workflow().edges}>
                          {(edge) => {
                            const from = () => workflow().nodes.find((node) => node.id === edge.from);
                            const to = () => workflow().nodes.find((node) => node.id === edge.to);
                            return (
                              <Show when={from() && to()}>
                                <path
                                  class="edge-path"
                                  d={buildEdgePath(from()!, to()!)}
                                  marker-end="url(#edge-arrow)"
                                />
                              </Show>
                            );
                          }}
                        </For>
                      </svg>
                      <For each={workflow().nodes}>
                        {(node) => {
                          const status = () => statusForNode(runState(), node.id);
                          return (
                            <button
                              class="node-card"
                              classList={{
                                selected: selectedNodeId() === node.id,
                                running: status() === "started",
                                waiting: status() === "awaiting_input",
                                done: status() === "completed",
                                failed: status() === "failed",
                              }}
                              style={{
                                transform: `translate(${node.position.x}px, ${node.position.y}px)`,
                              }}
                              onPointerDown={(event) => {
                                if (linkFromNodeId() && linkFromNodeId() !== node.id) {
                                  handleConnectOrSelectNode(node.id);
                                  return;
                                }
                                setSelectedNodeId(node.id);
                                setDragState({
                                  nodeId: node.id,
                                  pointerId: event.pointerId,
                                  startX: event.clientX,
                                  startY: event.clientY,
                                  originX: node.position.x,
                                  originY: node.position.y,
                                });
                                event.currentTarget.setPointerCapture(event.pointerId);
                              }}
                              onClick={() => handleConnectOrSelectNode(node.id)}
                            >
                              <div class="node-status-row">
                                <span class={`node-dot status-${status()}`} />
                                <span class="node-status-label">{labelForStatus(status())}</span>
                              </div>
                              <strong>{node.label}</strong>
                              <span class="node-model">{node.agent.model}</span>
                            </button>
                          );
                        }}
                      </For>
                    </>
                  )}
                </Show>
              </div>
            </section>

            <aside class="inspector-panel">
              <Show
                when={currentNode()}
                fallback={<div class="empty-panel">Select a node to edit its prompts, schema, and model.</div>}
              >
                {(node) => (
                  <>
                    <div class="panel-header">
                      <div>
                        <div class="eyebrow">Inspector</div>
                        <h3>{node().label}</h3>
                      </div>
                      <button class="danger-button" onClick={handleDeleteSelectedNode}>
                        Delete
                      </button>
                    </div>

                    <label>
                      <span>Label</span>
                      <input
                        class="text-input"
                        value={node().label}
                        onInput={(event) =>
                          updateActiveWorkflow((draft) => {
                            const nextNode = draft.nodes.find((item) => item.id === node().id);
                            if (nextNode) {
                              nextNode.label = event.currentTarget.value;
                            }
                          })
                        }
                      />
                    </label>

                    <label>
                      <span>Model</span>
                      <input
                        class="text-input"
                        list="known-models"
                        value={node().agent.model}
                        onInput={(event) =>
                          updateActiveWorkflow((draft) => {
                            const nextNode = draft.nodes.find((item) => item.id === node().id);
                            if (nextNode) {
                              nextNode.agent.model = event.currentTarget.value;
                            }
                          })
                        }
                      />
                      <datalist id="known-models">
                        <For each={activeProfileMemo().known_models}>{(model) => <option value={model} />}</For>
                      </datalist>
                    </label>

                    <label class="checkbox-row">
                      <input
                        type="checkbox"
                        checked={node().agent.auto_start}
                        onChange={(event) =>
                          updateActiveWorkflow((draft) => {
                            const nextNode = draft.nodes.find((item) => item.id === node().id);
                            if (nextNode) {
                              nextNode.agent.auto_start = event.currentTarget.checked;
                            }
                          })
                        }
                      />
                      <span>Auto-start without pausing for human input</span>
                    </label>

                    <label>
                      <span>System prompt</span>
                      <textarea
                        class="text-area"
                        rows={8}
                        value={node().agent.system_prompt}
                        onInput={(event) =>
                          updateActiveWorkflow((draft) => {
                            const nextNode = draft.nodes.find((item) => item.id === node().id);
                            if (nextNode) {
                              nextNode.agent.system_prompt = event.currentTarget.value;
                            }
                          })
                        }
                      />
                    </label>

                    <label>
                      <span>Task prompt</span>
                      <textarea
                        class="text-area"
                        rows={5}
                        value={node().agent.task_prompt}
                        onInput={(event) =>
                          updateActiveWorkflow((draft) => {
                            const nextNode = draft.nodes.find((item) => item.id === node().id);
                            if (nextNode) {
                              nextNode.agent.task_prompt = event.currentTarget.value;
                            }
                          })
                        }
                      />
                    </label>

                    <label>
                      <span>JSON output schema</span>
                      <textarea
                        class="text-area code"
                        rows={14}
                        value={schemaText()}
                        onInput={(event) => setSchemaText(event.currentTarget.value)}
                      />
                    </label>

                    <div class="button-row">
                      <button class="secondary-button" onClick={applySchemaEditor}>
                        Apply schema
                      </button>
                      <button class="secondary-button" onClick={handleBeginLink}>
                        Link from node
                      </button>
                    </div>

                    <div class="output-panel">
                      <div class="eyebrow">Latest output</div>
                      <pre>{currentNodeOutput() ? prettyJson(currentNodeOutput()) : "No output yet."}</pre>
                    </div>
                  </>
                )}
              </Show>
            </aside>
          </div>

          <section class="dock-panel">
            <div class="dock-tabs">
              <button classList={{ active: bottomTab() === "chat" }} onClick={() => setBottomTab("chat")}>
                Chat
              </button>
              <button classList={{ active: bottomTab() === "trace" }} onClick={() => setBottomTab("trace")}>
                Run trace
              </button>
              <div class="dock-spacer" />
              <button class="secondary-button small" onClick={() => void handleClearRunTrace()}>
                Clear run trace
              </button>
            </div>
            <Show
              when={bottomTab() === "chat"}
              fallback={
                <div class="trace-layout">
                  <div class="trace-list">
                    <For each={runState()?.runTrace ?? []}>
                      {(entry, index) => (
                        <button
                          class="trace-row"
                          classList={{ active: selectedTraceIndex() === index() }}
                          onClick={() => setSelectedTraceIndex(index())}
                        >
                          <span class={`trace-pill ${entry.status}`}>{entry.status.replace("_", " ")}</span>
                          <div>
                            <strong>{entry.nodeLabel}</strong>
                            <div>{entry.message}</div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                  <div class="trace-detail">
                    <Show when={selectedTrace()} fallback={<div class="empty-panel">Select a trace entry.</div>}>
                      {(entry) => (
                        <>
                          <div class="eyebrow">Trace detail</div>
                          <h3>{entry().nodeLabel}</h3>
                          <p>{entry().message}</p>
                          <pre>{entry().output ? prettyJson(entry().output) : "No output recorded."}</pre>
                        </>
                      )}
                    </Show>
                  </div>
                </div>
              }
            >
              <div class="chat-layout">
                <div class="chat-history">
                  <Show when={chatMessages().length > 0} fallback={<div class="empty-panel">Run a workflow or select a paused node to continue.</div>}>
                    <For each={chatMessages()}>
                      {(message) => (
                        <div class={`chat-row role-${message.role.toLowerCase()}`}>
                          <div class="chat-role">{message.role}</div>
                          <pre>{message.content}</pre>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
                <div class="chat-composer">
                  <div class="composer-meta">
                    <span>{readiness()?.provider ?? "Provider"}</span>
                    <span>
                      {runState()?.awaitingNodeId === selectedNodeId()
                        ? "Paused node selected"
                        : "Select the paused node to continue"}
                    </span>
                  </div>
                  <textarea
                    class="text-area"
                    rows={4}
                    value={chatInput()}
                    onInput={(event) => setChatInput(event.currentTarget.value)}
                    placeholder="Provide manual output for the paused node"
                    disabled={!chatEnabledMemo()}
                  />
                  <div class="button-row end">
                    <button class="primary-button" onClick={() => void handleSubmitChat()} disabled={!canSendChatMemo()}>
                      Send to node
                    </button>
                  </div>
                </div>
              </div>
            </Show>
          </section>
        </Show>
      </main>
    </div>
  );
}

function buildEdgePath(from: WorkflowNode, to: WorkflowNode) {
  const startX = from.position.x + NODE_WIDTH;
  const startY = from.position.y + NODE_HEIGHT / 2;
  const endX = to.position.x;
  const endY = to.position.y + NODE_HEIGHT / 2;
  const offset = Math.max((endX - startX) * 0.5, 64);
  return `M ${startX} ${startY} C ${startX + offset} ${startY}, ${endX - offset} ${endY}, ${endX} ${endY}`;
}

function labelForStatus(status: string) {
  switch (status) {
    case "queued":
      return "Queued";
    case "started":
      return "Running";
    case "awaiting_input":
      return "Waiting";
    case "completed":
      return "Done";
    case "failed":
      return "Failed";
    default:
      return "Idle";
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function normalizeError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return JSON.stringify(error);
}

function isTextInputTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable;
}

export default App;
