import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import Plus from "lucide-solid/icons/plus";
import TerminalSquare from "lucide-solid/icons/terminal";
import X from "lucide-solid/icons/x";
import { createEffect, For, onCleanup, onMount, Show } from "solid-js";
import { PanelEmptyState } from "../components/PanelEmptyState";
import { useAppContext } from "../context/AppContext";
import { readTerminalThemeColors } from "../lib/theme";
import { ICON_STROKE_WIDTH } from "../lib/utils";

const DEFAULT_COLS = 80;
const DEFAULT_ROWS = 24;

function terminalTabLabel(cwd: string | undefined): string {
  if (!cwd) return "Terminal";
  const normalized = cwd.replace(/\/+$/, "");
  return normalized.split("/").filter(Boolean).pop() ?? cwd;
}

function TerminalHost() {
  const ctx = useAppContext();
  let host: HTMLDivElement | undefined;
  let terminal: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let displayedSessionId: string | null = null;
  let writtenOutputLength = 0;

  const fitAndResize = () => {
    if (!terminal || !fitAddon) return;
    fitAddon.fit();
    void ctx.handleTerminalResize(terminal.cols, terminal.rows);
  };

  const syncDisplayedSession = (sessionId: string | null) => {
    if (!terminal || sessionId === displayedSessionId) return;
    displayedSessionId = sessionId;
    writtenOutputLength = 0;
    terminal.reset();
    if (!sessionId) return;
    const buffered = ctx.terminalOutputFor(sessionId);
    if (buffered) {
      terminal.write(buffered);
      writtenOutputLength = buffered.length;
    }
  };

  const applyTerminalTheme = () => {
    if (!terminal?.options) return;
    terminal.options.theme = readTerminalThemeColors();
  };

  onMount(() => {
    if (!host) return;
    terminal = new Terminal({
      cursorBlink: true,
      convertEol: true,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      scrollback: 5000,
      theme: readTerminalThemeColors(),
    });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    terminal.onData((data) => {
      const sessionId = ctx.activeTerminalSessionId();
      if (sessionId) {
        void ctx.handleTerminalInput(sessionId, data);
      }
    });
    fitAddon.fit();
    const cols = terminal.cols || DEFAULT_COLS;
    const rows = terminal.rows || DEFAULT_ROWS;
    syncDisplayedSession(ctx.activeTerminalSessionId());
    void ctx.handleTerminalResize(cols, rows);

    const observer = new ResizeObserver(() => fitAndResize());
    observer.observe(host);

    onCleanup(() => {
      observer.disconnect();
      terminal?.dispose();
      terminal = undefined;
      fitAddon = undefined;
      displayedSessionId = null;
      writtenOutputLength = 0;
    });
  });

  createEffect(() => {
    ctx.resolvedTheme();
    applyTerminalTheme();
  });

  createEffect(() => {
    syncDisplayedSession(ctx.activeTerminalSessionId());
  });

  createEffect(() => {
    const sessionId = ctx.activeTerminalSessionId();
    const output = sessionId ? ctx.terminalOutputFor(sessionId) : "";
    if (!terminal || !sessionId || sessionId !== displayedSessionId) return;
    if (output.length <= writtenOutputLength) return;
    terminal.write(output.slice(writtenOutputLength));
    writtenOutputLength = output.length;
  });

  createEffect(() => {
    const error = ctx.terminalError();
    if (error && terminal) {
      terminal.writeln(`\r\nTerminal error: ${error}`);
    }
  });

  return (
    <div class="terminal-host-shell">
      <div ref={host} class="terminal-host" />
      <Show when={ctx.terminalStarting()}>
        <div class="terminal-overlay">Starting terminal...</div>
      </Show>
    </div>
  );
}

export function TerminalPanel() {
  const ctx = useAppContext();

  onMount(() => {
    if (ctx.terminalSessions().length === 0 && !ctx.terminalStarting()) {
      void ctx.handleOpenTerminal(DEFAULT_COLS, DEFAULT_ROWS);
    }
  });

  const openNewTerminal = () => {
    void ctx.handleOpenTerminal(DEFAULT_COLS, DEFAULT_ROWS);
  };

  return (
    <div class="terminal-layout">
      <div class="terminal-tab-bar">
        <div class="terminal-tabs">
          <For each={ctx.terminalSessions()}>
            {(session) => (
              <div
                class="terminal-tab"
                classList={{ active: ctx.activeTerminalSessionId() === session.sessionId }}
              >
                <button
                  type="button"
                  class="terminal-tab-close"
                  onClick={() => void ctx.handleStopTerminal(session.sessionId)}
                  title="Close terminal"
                  aria-label={`Close ${terminalTabLabel(session.cwd)} terminal`}
                >
                  <X
                    width={12}
                    height={12}
                    aria-hidden="true"
                    absoluteStrokeWidth
                    strokeWidth={ICON_STROKE_WIDTH}
                  />
                </button>
                <button
                  type="button"
                  class="terminal-tab-select"
                  onClick={() => ctx.handleSelectTerminalSession(session.sessionId)}
                  title={session.cwd}
                >
                  <span class="terminal-tab-label">{terminalTabLabel(session.cwd)}</span>
                </button>
              </div>
            )}
          </For>
          <button
            type="button"
            class="terminal-tab-add"
            disabled={ctx.terminalStarting()}
            onClick={openNewTerminal}
            title="New terminal"
            aria-label="New terminal"
          >
            <Plus
              width={14}
              height={14}
              aria-hidden="true"
              absoluteStrokeWidth
              strokeWidth={ICON_STROKE_WIDTH}
            />
          </button>
        </div>
      </div>
      <Show
        when={ctx.terminalSessions().length > 0}
        fallback={
          <PanelEmptyState
            icon={<TerminalSquare width={22} height={22} />}
            title={ctx.terminalStarting() ? "Starting terminal..." : "No terminal open"}
            description={
              ctx.terminalStarting()
                ? undefined
                : "Press + to open a shell in the project directory."
            }
          />
        }
      >
        <TerminalHost />
      </Show>
    </div>
  );
}
