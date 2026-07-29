// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import type { ChatMessage, ToolCallSummary, WorkflowRunState } from "../../lib/types";
import { ConversationSegmentMessages } from "./ConversationSegmentMessages";
import { resetToolStackExpandStateForTests } from "./ToolStackBubble";

function toolMsg(id: string): ChatMessage {
  return { role: "Thinking", content: "", toolCallId: id };
}

function summary(
  id: string,
  toolName: string,
  argumentsValue: unknown = {},
  status: ToolCallSummary["status"] = "completed",
): ToolCallSummary {
  return {
    toolCallId: id,
    toolName,
    status,
    arguments: argumentsValue,
    lastOutput: "ok",
    isError: false,
    streaming: false,
  };
}

function stubContext(toolCalls: ToolCallSummary[]): AppContextValue {
  const runState = {
    active: false,
    toolCallsByNode: { "node-1": toolCalls },
  } as unknown as WorkflowRunState;
  return {
    runState: () => runState,
    executionCwdForActiveWorkflow: () => null,
  } as unknown as AppContextValue;
}

describe("ConversationSegmentMessages tool stacking", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    resetToolStackExpandStateForTests();
  });

  function renderSegment(messages: ChatMessage[], toolCalls: ToolCallSummary[]) {
    dispose = render(
      () => (
        <AppContext.Provider value={stubContext(toolCalls)}>
          <ConversationSegmentMessages
            nodeId="node-1"
            label="Agent"
            messages={messages}
          />
        </AppContext.Provider>
      ),
      container,
    );
  }

  it("renders one stack for >= 2 consecutive tools", () => {
    const messages = [toolMsg("a"), toolMsg("b")];
    renderSegment(messages, [summary("a", "read"), summary("b", "read")]);
    expect(container.querySelectorAll(".tool-stack")).toHaveLength(1);
    expect(container.textContent).toContain("Read 2 files");
    expect(container.querySelectorAll(".tool-stack .tool-line[data-tool-name]")).toHaveLength(0);
  });

  it("does not stack a single tool", () => {
    const messages = [toolMsg("a")];
    renderSegment(messages, [summary("a", "read")]);
    expect(container.querySelectorAll(".tool-stack")).toHaveLength(0);
    expect(container.querySelectorAll(".tool-line[data-tool-name='read']")).toHaveLength(1);
  });

  it("renders an attachment-only user message", () => {
    renderSegment(
      [
        {
          role: "User",
          content: "",
          attachments: [
            {
              id: "document-1",
              fileName: "brief.pdf",
              mediaType: "application/pdf",
              sizeBytes: 4096,
              sha256: "abc",
              kind: "document",
            },
          ],
        },
      ],
      [],
    );

    expect(container.querySelector(".message-user")).not.toBeNull();
    expect(container.textContent).toContain("brief.pdf");
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("keeps the prior checklist visible when a later update fails", () => {
    const failed = summary(
      "todo-2",
      "openflow_update_todo_list",
      { todos: [] },
      "failed",
    );
    failed.isError = true;
    failed.lastOutput = "todos must contain 1-12 items";
    renderSegment([toolMsg("todo-1"), toolMsg("todo-2")], [
      summary("todo-1", "openflow_update_todo_list", {
        todos: [{ content: "Implement checklist", status: "in_progress" }],
      }),
      failed,
    ]);

    expect(container.querySelectorAll(".todo-checklist")).toHaveLength(1);
    expect(container.textContent).toContain("Implement checklist");
    expect(
      container.querySelectorAll(
        ".tool-line[data-tool-name='openflow_update_todo_list'][data-status='failed']",
      ),
    ).toHaveLength(1);
  });

  it("renders only the latest todo update as a standalone checklist", () => {
    const messages = [toolMsg("todo-1"), toolMsg("read"), toolMsg("todo-2")];
    renderSegment(messages, [
      summary("todo-1", "openflow_update_todo_list", {
        todos: [{ content: "Trace behavior", status: "in_progress" }],
      }),
      summary("read", "read"),
      summary("todo-2", "openflow_update_todo_list", {
        todos: [
          { content: "Trace behavior", status: "completed" },
          { content: "Implement checklist", status: "in_progress" },
          { content: "Verify gates", status: "pending" },
        ],
      }),
    ]);

    expect(container.querySelectorAll(".todo-checklist")).toHaveLength(1);
    expect(container.querySelectorAll(".tool-stack")).toHaveLength(0);
    expect(container.textContent).not.toContain("Trace behaviorTrace behavior");
    expect(container.textContent).toContain("Progress1/3");
    expect(container.textContent).toContain("Implement checklist");
    expect(container.querySelectorAll(".tool-line[data-tool-name='read']")).toHaveLength(1);
  });

  it("merges tool batches separated only by thinking into one stack", () => {
    const messages = [
      toolMsg("a"),
      toolMsg("b"),
      toolMsg("c"),
      { role: "Thinking", content: "hmm" } satisfies ChatMessage,
      toolMsg("d"),
      toolMsg("e"),
      toolMsg("f"),
    ];
    renderSegment(messages, [
      summary("a", "read"),
      summary("b", "read"),
      summary("c", "read"),
      summary("d", "search"),
      summary("e", "search"),
      summary("f", "search"),
    ]);
    expect(container.querySelectorAll(".tool-stack")).toHaveLength(1);
    expect(container.textContent).toContain(
      "Read 3 files · Grepped 3 patterns · Thought for a while",
    );
    expect(container.querySelectorAll(".tool-line--thinking")).toHaveLength(0);
    container.querySelector<HTMLElement>(".tool-stack-status-row")?.click();
    container
      .querySelector<HTMLButtonElement>(".tool-line--thinking .tool-line-status-row")
      ?.click();
    expect(container.textContent).toContain("hmm");
  });

  it("renders assistant think tags as ThinkingBubble inside the stack", () => {
    const messages = [
      toolMsg("a"),
      toolMsg("b"),
      { role: "Assistant", content: "<think>\nnext batch\n</think>" } satisfies ChatMessage,
      toolMsg("c"),
    ];
    renderSegment(messages, [
      summary("a", "read"),
      summary("b", "read"),
      summary("c", "read"),
    ]);
    expect(container.querySelectorAll(".tool-stack")).toHaveLength(1);
    expect(container.textContent).toContain("Read 3 files · Thought for a while");
    expect(container.textContent).not.toContain("next batch");
    container.querySelector<HTMLElement>(".tool-stack-status-row")?.click();
    expect(container.querySelectorAll(".tool-line--thinking")).toHaveLength(1);
    container
      .querySelector<HTMLButtonElement>(".tool-line--thinking .tool-line-status-row")
      ?.click();
    expect(container.textContent).toContain("next batch");
  });
});
