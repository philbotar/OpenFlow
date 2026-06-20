import { For, Show } from "solid-js";
import LayoutList from "lucide-solid/icons/layout-list";
import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import { ChatPanel } from "../components/conversation";
import { PanelEmptyState } from "../components/PanelEmptyState";
import { useAppContext } from "../context/AppContext";
import { prettyJson } from "../lib/workflow";
import { TerminalPanel } from "./TerminalPanel";
import { RunHistoryPanel } from "./RunHistoryPanel";

export function DockPanel() {
  const ctx = useAppContext();

  return (
    <section class="dock-panel" classList={{ collapsed: !ctx.dockOpen() }}>
      <div
        class="dock-resize-zone"
        onPointerDown={ctx.handleDockResizePointerDown}
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize bottom panel"
      />
      <div class="dock-tabs">
        <div class="dock-tab-switcher">
          <button
            classList={{ active: ctx.bottomTab() === "overview" }}
            onClick={() => ctx.handleSelectBottomTab("overview")}
          >
            Overview
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "chat" }}
            onClick={() => ctx.handleSelectBottomTab("chat")}
          >
            Chat
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "terminal" }}
            onClick={() => ctx.handleSelectBottomTab("terminal")}
          >
            Terminal
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "trace" }}
            onClick={() => ctx.handleSelectBottomTab("trace")}
          >
            Run trace
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "runs" }}
            onClick={() => ctx.handleSelectBottomTab("runs")}
          >
            Runs
          </button>
        </div>
        <Show
          when={
            ctx.dockOpen() &&
            (ctx.bottomTab() === "chat" ||
              ctx.bottomTab() === "trace" ||
              ctx.bottomTab() === "terminal" ||
              ctx.bottomTab() === "runs")
          }
        >
          <div class="dock-tab-actions">
            <Show when={ctx.bottomTab() === "chat"}>
              <button
                type="button"
                class="dock-icon-action dock-focus-action"
                title={ctx.chatFocusMode() ? "Show canvas" : "Focus chat"}
                aria-label={ctx.chatFocusMode() ? "Show canvas" : "Focus chat"}
                aria-pressed={ctx.chatFocusMode()}
                onClick={() => ctx.handleToggleChatFocusMode()}
              >
                <Show when={ctx.chatFocusMode()} fallback={<Maximize2 width={15} height={15} />}>
                  <Minimize2 width={15} height={15} />
                </Show>
              </button>
            </Show>
            <Show when={ctx.bottomTab() === "trace" && ctx.hasRunTraceMemo()}>
              <button
                class="secondary-button small ghost dock-trace-action"
                onClick={() => void ctx.handleClearRunTrace()}
              >
                Clear trace
              </button>
            </Show>
          </div>
        </Show>
      </div>

      <Show when={ctx.dockOpen()}>
        <Show when={ctx.bottomTab() === "overview"} fallback={<DockTabContent />}>
          <div class="overview-layout">
            <div class="overview-feed">
              <Show
                when={(ctx.runState()?.runTrace?.length ?? 0) > 0}
                fallback={
                  <PanelEmptyState
                    icon={<LayoutList width={22} height={22} />}
                    title="No activity yet"
                    description="Run the workflow to see node progress here."
                  />
                }
              >
                <For each={ctx.runState()?.runTrace ?? []}>
                  {(entry) => (
                    <div class="overview-entry">
                      <div class="overview-node-label">{entry.nodeLabel}</div>
                      <div class="overview-status">
                        {entry.status.replace("_", " ")}
                      </div>
                      <div class="overview-message">{entry.message}</div>
                      <Show when={entry.output}>
                        <pre class="overview-output">{prettyJson(entry.output)}</pre>
                      </Show>
                    </div>
                  )}
                </For>
              </Show>
            </div>
          </div>
        </Show>
      </Show>
    </section>
  );
}

function DockTabContent() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "chat"} fallback={<TerminalOrTrace />}>
      <ChatPanel />
    </Show>
  );
}

function TerminalOrTrace() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "terminal"} fallback={<TraceOrRuns />}>
      <TerminalPanel />
    </Show>
  );
}

function TraceOrRuns() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "runs"} fallback={<TracePanel />}>
      <RunHistoryPanel />
    </Show>
  );
}

function TracePanel() {
  const ctx = useAppContext();

  return (
    <div class="trace-layout">
      <div class="trace-list">
        <For each={ctx.runState()?.runTrace ?? []}>
          {(entry, index) => (
            <button
              class="trace-row"
              classList={{ active: ctx.selectedTraceIndex() === index() }}
              onClick={() => ctx.setSelectedTraceIndex(index())}
            >
              <span class={`trace-pill ${entry.status}`}>
                {entry.status.replace("_", " ")}
              </span>
              <div>
                <strong>{entry.nodeLabel}</strong>
                <div>{entry.message}</div>
              </div>
            </button>
          )}
        </For>
      </div>
      <div class="trace-detail">
        <Show
          when={ctx.selectedTrace()}
          fallback={
            <PanelEmptyState
              title="Select a trace entry"
              description="Choose an event from the list to inspect its output."
            />
          }
        >
          {(entry) => (
            <>
              <div class="eyebrow">Trace detail</div>
              <h3>{entry().nodeLabel}</h3>
              <p>{entry().message}</p>
              <pre>
                {entry().output ? prettyJson(entry().output) : "No output recorded."}
              </pre>
            </>
          )}
        </Show>
      </div>
    </div>
  );
}
