import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { createEffect, onCleanup, onMount, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";

const DEFAULT_COLS = 80;
const DEFAULT_ROWS = 24;

export function TerminalPanel() {
  const ctx = useAppContext();
  let host: HTMLDivElement | undefined;
  let terminal: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let lastSessionId: string | null = null;
  let writtenOutputLength = 0;

  const fitAndResize = () => {
    if (!terminal || !fitAddon) return;
    fitAddon.fit();
    void ctx.handleTerminalResize(terminal.cols, terminal.rows);
  };

  onMount(() => {
    if (!host) return;
    terminal = new Terminal({
      cursorBlink: true,
      convertEol: true,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      scrollback: 5000,
      theme: {
        background: "#101114",
        foreground: "#f3f4f6",
        cursor: "#f3f4f6",
        selectionBackground: "#4b5563",
      },
    });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    terminal.onData((data) => {
      void ctx.handleTerminalInput(data);
    });
    fitAddon.fit();
    void ctx.handleOpenTerminal(terminal.cols || DEFAULT_COLS, terminal.rows || DEFAULT_ROWS);

    const observer = new ResizeObserver(() => fitAndResize());
    observer.observe(host);

    onCleanup(() => {
      observer.disconnect();
      terminal?.dispose();
      terminal = undefined;
      fitAddon = undefined;
      lastSessionId = null;
      writtenOutputLength = 0;
    });
  });

  createEffect(() => {
    const event = ctx.terminalSession();
    if (!event || !terminal) return;
    if (lastSessionId !== event.sessionId) {
      lastSessionId = event.sessionId;
      writtenOutputLength = 0;
      terminal.reset();
      terminal.writeln(`OpenFlow terminal: ${event.cwd}`);
      terminal.writeln("");
    }
  });

  createEffect(() => {
    const output = ctx.terminalOutput();
    if (!terminal || output.length <= writtenOutputLength) return;
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
    <div class="terminal-layout">
      <div class="terminal-toolbar">
        <div>
          <div class="eyebrow">Terminal</div>
          <div class="terminal-cwd">
            {ctx.terminalSession()?.cwd ?? ctx.executionCwdForActiveWorkflow() ?? "Process cwd"}
          </div>
        </div>
        <button
          class="secondary-button small ghost"
          disabled={!ctx.terminalSession()}
          onClick={() => void ctx.handleStopTerminal()}
        >
          Stop
        </button>
      </div>
      <div class="terminal-host-shell">
        <div ref={host} class="terminal-host" />
        <Show when={ctx.terminalStarting()}>
          <div class="terminal-overlay">Starting terminal...</div>
        </Show>
      </div>
    </div>
  );
}
