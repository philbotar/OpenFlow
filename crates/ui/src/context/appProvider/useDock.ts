import { createSignal, type Accessor } from "solid-js";
import * as desktop from "../../api";
import type { BottomTab, TerminalEvent, TerminalStart } from "../../lib/types";
import {
  clampDockHeight,
  COLLAPSED_DOCK_HEIGHT,
  DEFAULT_DOCK_HEIGHT,
  normalizeError,
  restoredChatDockHeight,
  shouldCollapseDock,
  viewportHeight,
} from "../../lib/utils";

interface UseDockParams {
  executionCwdForActiveWorkflow: Accessor<string | null>;
  isCompactViewport: Accessor<boolean>;
}

export function useDock(params: UseDockParams) {
  const [bottomTab, setBottomTab] = createSignal<BottomTab>("overview");
  const [dockOpen, setDockOpen] = createSignal(true);
  const [dockHeight, setDockHeight] = createSignal(DEFAULT_DOCK_HEIGHT);
  const [chatFocusMode, setChatFocusMode] = createSignal(false);
  const [terminalSessions, setTerminalSessions] = createSignal<TerminalStart[]>([]);
  const [activeTerminalSessionId, setActiveTerminalSessionId] = createSignal<string | null>(null);
  const [terminalStarting, setTerminalStarting] = createSignal(false);
  const [terminalError, setTerminalError] = createSignal<string | null>(null);
  const [terminalOutputs, setTerminalOutputs] = createSignal<Record<string, string>>({});

  let dockResizeState: { startY: number; startHeight: number } | null = null;

  const focusChatDock = () => {
    setDockOpen(true);
    setBottomTab("chat");
    setDockHeight((current) => clampDockHeight(current, "chat"));
  };

  const handleSelectBottomTab = (tab: BottomTab) => {
    const wasCollapsed = !dockOpen();
    setBottomTab(tab);
    setDockOpen(true);
    if (chatFocusMode()) {
      return;
    }
    if (tab === "chat" && wasCollapsed) {
      setDockHeight(restoredChatDockHeight(viewportHeight(), params.isCompactViewport()));
      return;
    }
    setDockHeight((current) => clampDockHeight(current, tab));
  };

  const handleToggleChatFocusMode = () => {
    setDockOpen(true);
    if (chatFocusMode()) {
      setDockHeight(restoredChatDockHeight(viewportHeight(), params.isCompactViewport()));
    }
    setChatFocusMode((current) => !current);
  };

  const handleDockResizePointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    dockResizeState = {
      startY: event.clientY,
      startHeight: dockOpen() ? dockHeight() : COLLAPSED_DOCK_HEIGHT,
    };
    document.body.classList.add("is-resizing-dock");
  };

  const handleDockResizePointerMove = (event: PointerEvent) => {
    if (!dockResizeState) return;
    const nextHeight = dockResizeState.startHeight + (dockResizeState.startY - event.clientY);
    if (shouldCollapseDock(nextHeight, bottomTab(), params.isCompactViewport())) {
      setDockOpen(false);
      return;
    }
    setDockOpen(true);
    setDockHeight(clampDockHeight(nextHeight, bottomTab()));
  };

  const clearDockResizeState = () => {
    if (!dockResizeState) return;
    dockResizeState = null;
    document.body.classList.remove("is-resizing-dock");
  };

  const terminalOutputFor = (sessionId: string) => terminalOutputs()[sessionId] ?? "";

  const removeTerminalSession = (sessionId: string) => {
    setTerminalSessions((sessions) => {
      const next = sessions.filter((session) => session.sessionId !== sessionId);
      if (activeTerminalSessionId() === sessionId) {
        setActiveTerminalSessionId(next.length > 0 ? next[next.length - 1].sessionId : null);
      }
      return next;
    });
    setTerminalOutputs((outputs) => {
      const { [sessionId]: _removed, ...rest } = outputs;
      return rest;
    });
  };

  const handleOpenTerminal = async (cols: number, rows: number) => {
    if (terminalStarting()) return;
    setTerminalStarting(true);
    setTerminalError(null);
    try {
      const session = await desktop.startTerminal(
        params.executionCwdForActiveWorkflow(),
        cols,
        rows,
      );
      setTerminalOutputs((outputs) => ({ ...outputs, [session.sessionId]: "" }));
      setTerminalSessions((sessions) => [...sessions, session]);
      setActiveTerminalSessionId(session.sessionId);
    } catch (error) {
      setTerminalError(normalizeError(error));
    } finally {
      setTerminalStarting(false);
    }
  };

  const handleSelectTerminalSession = (sessionId: string) => {
    if (!terminalSessions().some((session) => session.sessionId === sessionId)) return;
    setActiveTerminalSessionId(sessionId);
  };

  const handleTerminalInput = async (sessionId: string, data: string) => {
    if (!terminalSessions().some((session) => session.sessionId === sessionId)) return;
    try {
      await desktop.writeTerminal(sessionId, data);
    } catch (error) {
      setTerminalError(normalizeError(error));
    }
  };

  const handleTerminalResize = async (cols: number, rows: number) => {
    const sessionId = activeTerminalSessionId();
    if (!sessionId) return;
    try {
      await desktop.resizeTerminal(sessionId, cols, rows);
    } catch (error) {
      setTerminalError(normalizeError(error));
    }
  };

  const handleStopTerminal = async (sessionId?: string) => {
    const targets = sessionId
      ? terminalSessions().filter((session) => session.sessionId === sessionId)
      : terminalSessions();
    if (targets.length === 0) return;
    try {
      await Promise.all(targets.map((session) => desktop.stopTerminal(session.sessionId)));
    } catch (error) {
      setTerminalError(normalizeError(error));
    } finally {
      if (sessionId) {
        removeTerminalSession(sessionId);
        return;
      }
      setTerminalSessions([]);
      setActiveTerminalSessionId(null);
      setTerminalOutputs({});
    }
  };

  const handleTerminalEvent = (event: TerminalEvent) => {
    if (!terminalSessions().some((session) => session.sessionId === event.sessionId)) return;
    const { kind } = event;
    switch (kind.type) {
      case "output":
        setTerminalOutputs((outputs) => ({
          ...outputs,
          [event.sessionId]: (outputs[event.sessionId] ?? "") + kind.data,
        }));
        return;
      case "error":
        setTerminalError(kind.message);
        return;
      case "exit":
        removeTerminalSession(event.sessionId);
    }
  };

  return {
    bottomTab,
    setBottomTab,
    dockOpen,
    setDockOpen,
    dockHeight,
    setDockHeight,
    chatFocusMode,
    setChatFocusMode,
    terminalSessions,
    activeTerminalSessionId,
    terminalStarting,
    terminalError,
    terminalOutputFor,
    focusChatDock,
    handleSelectBottomTab,
    handleToggleChatFocusMode,
    handleDockResizePointerDown,
    handleDockResizePointerMove,
    clearDockResizeState,
    handleOpenTerminal,
    handleSelectTerminalSession,
    handleTerminalInput,
    handleTerminalResize,
    handleStopTerminal,
    handleTerminalEvent,
  };
}
