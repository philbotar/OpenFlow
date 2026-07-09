import { For, Show } from "solid-js";

import appIcon from "../../../../desktop/icons/icon.png";

const ICON_SRC = appIcon;

export function IntroOverlay(props: { onDismiss: () => void }) {
  return (
    <div
      class="of-intro-overlay"
      data-testid="first-run-onboarding-intro"
      onClick={props.onDismiss}
    >
      <svg class="of-intro-gradients" aria-hidden="true">
        <defs>
          <linearGradient id="of-rg1" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#cce6f8" stop-opacity="1" />
            <stop offset="100%" stop-color="#2770d4" stop-opacity="1" />
          </linearGradient>
          <linearGradient id="of-rg2" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#a4ceef" stop-opacity="0.85" />
            <stop offset="100%" stop-color="#1a5ec0" stop-opacity="0.85" />
          </linearGradient>
          <linearGradient id="of-rg3" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stop-color="#7db8e8" stop-opacity="0.65" />
            <stop offset="100%" stop-color="#4a8fd6" stop-opacity="0.65" />
          </linearGradient>
        </defs>
      </svg>

      <div class="of-intro-glow" />

      <WaveRing gradient="of-rg1" strokeWidth={2.5} delay="0.2s" direction="out" />
      <WaveRing gradient="of-rg2" strokeWidth={2.0} delay="0.45s" direction="out" />
      <WaveRing gradient="of-rg3" strokeWidth={1.5} delay="0.7s" direction="out" />
      <WaveRing gradient="of-rg1" strokeWidth={2.5} delay="2.2s" direction="in" />
      <WaveRing gradient="of-rg2" strokeWidth={2.0} delay="2.45s" direction="in" />
      <WaveRing gradient="of-rg3" strokeWidth={1.5} delay="2.7s" direction="in" />

      <div class="of-intro-flash" />

      <div class="of-intro-icon-wrap">
        <div class="of-intro-icon">
          <img src={ICON_SRC} class="of-img-fill" alt="Openflow" draggable={false} />
        </div>
      </div>
    </div>
  );
}

function WaveRing(props: {
  gradient: string;
  strokeWidth: number;
  delay: string;
  direction: "out" | "in";
}) {
  return (
    <svg
      class={`of-wave-ring of-wave-ring--${props.direction}`}
      style={{ "animation-delay": props.delay }}
      aria-hidden="true"
    >
      <circle
        cx="55"
        cy="55"
        r="52"
        fill="none"
        stroke={`url(#${props.gradient})`}
        stroke-width={props.strokeWidth}
      />
    </svg>
  );
}

export function SlideWelcome() {
  return (
    <div class="of-slide of-slide-welcome">
      <div class="of-slide-glow-primary" />
      <div class="of-slide-glow-secondary" />
      <div class="of-slide-welcome-content">
        <div class="of-slide-icon-wrap">
          <div class="of-slide-icon">
            <img src={ICON_SRC} class="of-img-fill" alt="Openflow" draggable={false} />
          </div>
        </div>
        <div class="of-slide-kicker">Welcome to</div>
        <h1 class="of-slide-title-hero">Openflow</h1>
        <p class="of-slide-lead">
          Build repeatable AI workflows.
          <br />
          Describe a goal, get something you can inspect, run, and reuse.
        </p>
      </div>
    </div>
  );
}

export function SlideDescribe() {
  return (
    <div class="of-slide of-slide-split">
      <div class="of-slide-copy">
        <Eyebrow label="01 — DESCRIBE" />
        <h1 class="of-slide-heading">
          Explain your
          <br />
          workflow.
        </h1>
        <p class="of-slide-body">
          Describe what you want to accomplish. Openflow designs the workflow — nodes, edges,
          instructions — ready to inspect before you run.
        </p>
      </div>
      <div class="of-mock-showcase">
        <MockWindow title="Build with AI">
          <div class="of-mock-body">
            <div class="of-mock-draft-pane">
              <div class="of-mock-pane-label">DRAFT PREVIEW</div>
              <For each={["Plan", "Research", "Draft"]}>
                {(label, i) => (
                  <>
                    <Show when={i() > 0}>
                      <div class="of-mock-node-connector">
                        <div class="of-mock-node-connector-line" />
                      </div>
                    </Show>
                    <MockNode label={label} />
                  </>
                )}
              </For>
              <div class="of-mock-create-btn">
                <div class="of-mock-create-btn-inner">Create Workflow</div>
              </div>
            </div>
            <div class="of-mock-chat-pane">
              <div class="of-mock-chat-messages">
                <div class="of-mock-user-bubble">
                  <div class="of-mock-user-bubble-text">
                    Research competitors for a PM tool targeting early-stage startups
                  </div>
                </div>
                <div class="of-mock-status-row">
                  <div class="of-spinner of-spinner--sm" />
                  <span class="of-mock-status-text">Building workflow draft…</span>
                </div>
              </div>
              <MockComposer />
            </div>
          </div>
        </MockWindow>
      </div>
    </div>
  );
}

export function SlideInspect() {
  return (
    <div class="of-slide of-slide-split">
      <div class="of-slide-copy">
        <Eyebrow label="02 — INSPECT" />
        <h1 class="of-slide-heading">
          See every
          <br />
          step.
        </h1>
        <p class="of-slide-body">
          Your goal becomes a transparent, editable graph. Read each instruction, swap models,
          reorder steps.
        </p>
      </div>
      <div class="of-mock-showcase">
        <MockWindow title="Competitor research brief">
          <div class="of-mock-body">
            <div class="of-mock-sidebar">
              <div class="of-mock-sidebar-label">WORKFLOWS</div>
              <div class="of-mock-sidebar-item">New workflow</div>
              <div class="of-mock-sidebar-item is-active">Competitor r…</div>
            </div>
            <div class="of-mock-main">
              <div class="of-mock-canvas">
                <div class="of-mock-canvas-nodes">
                  <div class="of-mock-graph-node is-selected">
                    <div class="of-mock-node-status">IDLE</div>
                    <div class="of-mock-node-label">Plan</div>
                  </div>
                  <CanvasEdge />
                  <div class="of-mock-graph-node">
                    <div class="of-mock-node-status">IDLE</div>
                    <div class="of-mock-node-label">Research</div>
                  </div>
                </div>
              </div>
              <div class="of-mock-dock">
                <DockTabs active="Chat" />
                <div class="of-mock-dock-empty">
                  <div class="of-mock-dock-empty-title">No messages yet</div>
                  <div class="of-mock-dock-empty-sub">Send a message to start the workflow.</div>
                </div>
              </div>
            </div>
          </div>
        </MockWindow>
      </div>
    </div>
  );
}

export function SlideRun() {
  return (
    <div class="of-slide of-slide-split">
      <div class="of-slide-copy">
        <Eyebrow label="03 — RUN" />
        <h1 class="of-slide-heading">
          Just send
          <br />a message.
        </h1>
        <p class="of-slide-body">
          Send a message to kick off the workflow. Nodes execute in order — output, tool approvals,
          and any pauses all happen right here in the dock.
        </p>
      </div>
      <div class="of-mock-showcase">
        <MockWindow title="Competitor research brief" stopBadge>
          <div class="of-mock-body">
            <div class="of-mock-sidebar of-mock-sidebar--narrow">
              <div class="of-mock-sidebar-label">WORKFLOWS</div>
              <div class="of-mock-sidebar-item">New workflow</div>
              <div class="of-mock-sidebar-item is-active of-mock-sidebar-item--compact">
                Competitor…
              </div>
            </div>
            <div class="of-mock-main">
              <div class="of-mock-canvas of-mock-canvas--short">
                <div class="of-mock-canvas-nodes of-mock-canvas-nodes--run">
                  <For each={["Plan", "Research", "Draft"]}>
                    {(label, i) => (
                      <>
                        <Show when={i() > 0}>
                          <CanvasEdge small />
                        </Show>
                        <div class="of-mock-graph-node of-mock-graph-node--sm">
                          <div class="of-mock-node-status of-mock-node-status--sm">IDLE</div>
                          <div class="of-mock-node-label of-mock-node-label--sm">{label}</div>
                        </div>
                      </>
                    )}
                  </For>
                </div>
              </div>
              <div class="of-mock-dock-panel">
                <DockTabs active="Chat" />
                <div class="of-mock-dock-center">
                  <div class="of-mock-dock-center-title">No messages yet</div>
                  <div class="of-mock-dock-center-sub">Send a message to start the workflow.</div>
                </div>
                <div class="of-mock-compose-bar">
                  <div class="of-mock-compose of-mock-compose--highlight">
                    <div class="of-mock-compose-text">
                      Write a competitor brief for Notion vs Linear…
                    </div>
                    <SendButton />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </MockWindow>
      </div>
    </div>
  );
}

export function SlideDone(props: { onBuildWorkflow: () => void; onSetupProvider: () => void }) {
  return (
    <div class="of-slide of-slide-done">
      <div class="of-slide-done-glow" />
      <div class="of-slide-done-content">
        <div class="of-slide-done-icon-wrap">
          <div class="of-slide-done-icon">
            <img src={ICON_SRC} class="of-img-fill" alt="Openflow" draggable={false} />
          </div>
        </div>
        <h1 class="of-slide-done-title">
          One thing before
          <br />
          you start.
        </h1>
        <p class="of-slide-done-lead">
          You'll need an AI provider key to run workflows. Add one in Settings — it takes 30
          seconds.
        </p>
        <p class="of-slide-done-note">Anthropic, OpenAI, and others are supported.</p>
        <div class="of-slide-done-actions">
          <button type="button" class="of-btn-primary" onClick={props.onSetupProvider}>
            Set up provider →
          </button>
          <button type="button" class="of-btn-secondary" onClick={props.onBuildWorkflow}>
            Build with AI
          </button>
        </div>
        <p class="of-slide-done-footnote">You can also add a key later from Settings.</p>
      </div>
    </div>
  );
}

function Eyebrow(props: { label: string }) {
  return (
    <div class="of-eyebrow">
      <span class="of-eyebrow-label">{props.label}</span>
    </div>
  );
}

function MockWindow(props: { title: string; stopBadge?: boolean; children: any }) {
  return (
    <div class="of-mock-window">
      <div class="of-mock-titlebar">
        <div class="of-mock-traffic">
          <For each={[0, 1, 2]}>{() => <div class="of-mock-traffic-dot" />}</For>
        </div>
        <span class="of-mock-title">{props.title}</span>
        <Show when={props.stopBadge}>
          <div class="of-mock-stop-badge">Stop</div>
        </Show>
      </div>
      {props.children}
    </div>
  );
}

function MockNode(props: { label: string }) {
  return (
    <div class="of-mock-node">
      <div class="of-mock-node-head">
        <div class="of-mock-node-dot" />
        <span class="of-mock-node-idle">IDLE</span>
      </div>
      <div class="of-mock-node-name">{props.label}</div>
    </div>
  );
}

function CanvasEdge(props: { small?: boolean }) {
  return (
    <div class="of-canvas-edge">
      <div class="of-canvas-edge-dot" />
      <div class={`of-canvas-edge-line ${props.small ? "of-canvas-edge-line--sm" : "of-canvas-edge-line--md"}`} />
      <div class="of-canvas-edge-arrow" />
      <div class="of-canvas-edge-dot" />
    </div>
  );
}

function DockTabs(props: { active: string }) {
  return (
    <div class="of-dock-tabs">
      <For each={["Chat", "Terminal", "Run trace"]}>
        {(tab) => (
          <div class={`of-dock-tab ${tab === props.active ? "is-active" : ""}`}>{tab}</div>
        )}
      </For>
    </div>
  );
}

function MockComposer() {
  return (
    <div class="of-mock-compose-bar">
      <div class="of-mock-compose">
        <div class="of-mock-compose-placeholder" />
        <SendButton />
      </div>
    </div>
  );
}

function SendButton() {
  return (
    <div class="of-send-btn">
      <div class="of-send-btn-arrow" />
    </div>
  );
}
