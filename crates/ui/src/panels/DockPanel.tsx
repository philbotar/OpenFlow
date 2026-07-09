import { For, Show } from "solid-js";
import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import { ChatPanel, PanelEmptyState } from "@/components";
import { useAppContext } from "../context/AppContext";
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
            classList={{ active: ctx.bottomTab() === "history" }}
            onClick={() => ctx.handleSelectBottomTab("history")}
          >
            History
          </button>
        </div>
        <Show when={ctx.dockOpen()}>
          <div class="dock-tab-actions">
            <button
              type="button"
              class="dock-icon-action dock-focus-action"
              title={ctx.chatFocusMode() ? "Show canvas" : "Focus panel"}
              aria-label={ctx.chatFocusMode() ? "Show canvas" : "Focus panel"}
              aria-pressed={ctx.chatFocusMode()}
              onClick={() => ctx.handleToggleChatFocusMode()}
            >
              <Show when={ctx.chatFocusMode()} fallback={<Maximize2 width={15} height={15} />}>
                <Minimize2 width={15} height={15} />
              </Show>
            </button>
          </div>
        </Show>
      </div>

      <Show when={ctx.dockOpen()}>
        <DockTabContent />
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
    <Show when={ctx.bottomTab() === "terminal"} fallback={<TraceOrHistory />}>
      <TerminalPanel />
    </Show>
  );
}

function TraceOrHistory() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "history"} fallback={<TracePanel />}>
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
                {entry().output ? JSON.stringify(entry().output, null, 2) : "No output recorded."}
              </pre>
            </>
          )}
        </Show>
      </div>
    </div>
  );
}
