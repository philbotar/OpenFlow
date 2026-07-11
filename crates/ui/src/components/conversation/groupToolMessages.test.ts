// @vitest-environment node
import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../../lib/types";
import { groupToolMessages } from "./groupToolMessages";

function tool(id: string): ChatMessage {
  return { role: "Thinking", content: "", toolCallId: id };
}

function text(content: string): ChatMessage {
  return { role: "Assistant", content };
}

function thinking(content: string): ChatMessage {
  return { role: "Thinking", content };
}

describe("groupToolMessages", () => {
  it("leaves a single tool as an individual item", () => {
    const messages = [tool("a")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "tool", message: messages[0] },
    ]);
  });

  it("stacks consecutive runs of length >= 2", () => {
    const messages = [tool("a"), tool("b")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("stacks consecutive runs of length >= 3", () => {
    const messages = [tool("a"), tool("b"), tool("c")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("breaks a run on assistant text", () => {
    const messages = [tool("a"), tool("b"), tool("c"), text("hi"), tool("d"), tool("e"), tool("f")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages: messages.slice(0, 3) },
      { kind: "message", message: messages[3] },
      { kind: "toolStack", messages: messages.slice(4) },
    ]);
  });

  it("folds thinking into the stack when tools continue", () => {
    const messages = [tool("a"), tool("b"), thinking("…"), tool("c")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("merges multiple tool batches separated only by thinking", () => {
    const messages = [
      tool("a"),
      tool("b"),
      tool("c"),
      thinking("plan next"),
      tool("d"),
      tool("e"),
      tool("f"),
    ];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("breaks a run on node_completed", () => {
    const done: ChatMessage = {
      role: "Assistant",
      content: "done",
      messageKind: "node_completed",
    };
    const messages = [tool("a"), tool("b"), tool("c"), done];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages: messages.slice(0, 3) },
      { kind: "message", message: done },
    ]);
  });

  it("preserves non-tool order around a single tool", () => {
    const messages = [text("before"), tool("a"), text("after")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "message", message: messages[0] },
      { kind: "tool", message: messages[1] },
      { kind: "message", message: messages[2] },
    ]);
  });

  it("grows a live stack when a fourth tool appends", () => {
    const three = [tool("a"), tool("b"), tool("c")];
    expect(groupToolMessages(three)[0]).toMatchObject({ kind: "toolStack" });
    const four = [...three, tool("d")];
    const items = groupToolMessages(four);
    expect(items).toHaveLength(1);
    expect(items[0]).toEqual({ kind: "toolStack", messages: four });
  });

  it("skips empty assistant/thinking so they do not break or appear", () => {
    const messages = [
      tool("a"),
      tool("b"),
      text(""),
      thinking(""),
      tool("c"),
    ];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages: [messages[0], messages[1], messages[4]] },
    ]);
  });

  it("skips assistant content that is only tool-call markup", () => {
    const markup = text(
      "<tool_call>\n<function=search>\n</function>\n</tool_call>",
    );
    const messages = [tool("a"), tool("b"), markup, tool("c")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages: [messages[0], messages[1], messages[3]] },
    ]);
  });

  it("turns think-only assistant into ThinkingBubble and folds it into the stack", () => {
    const thinkOnly = text("<think>\nnext batch\n</think>");
    const approval: ChatMessage = {
      role: "system",
      content: "Approval required for tool 'bash'.",
    };
    const messages = [
      tool("a"),
      tool("b"),
      thinkOnly,
      approval,
      tool("c"),
      tool("d"),
      tool("e"),
    ];
    const items = groupToolMessages(messages);
    expect(items).toHaveLength(1);
    expect(items[0]?.kind).toBe("toolStack");
    if (items[0]?.kind !== "toolStack") return;
    expect(items[0].messages).toEqual([
      messages[0],
      messages[1],
      { role: "Thinking", content: "next batch" },
      messages[4],
      messages[5],
      messages[6],
    ]);
  });

  it("splits think + prose: thinking bridges, prose breaks the stack", () => {
    const withProse = text("<think>\nhmm\n</think>\nJS inventory complete.");
    const messages = [tool("a"), tool("b"), tool("c"), withProse, tool("d"), tool("e"), tool("f")];
    expect(groupToolMessages(messages)).toEqual([
      {
        kind: "toolStack",
        messages: [
          messages[0],
          messages[1],
          messages[2],
          { role: "Thinking", content: "hmm" },
        ],
      },
      { kind: "message", message: { ...withProse, content: "JS inventory complete." } },
      { kind: "toolStack", messages: messages.slice(4) },
    ]);
  });

  it("keeps consecutive thinking bubbles separate (no merge)", () => {
    const messages = [
      tool("a"),
      tool("b"),
      thinking("first"),
      thinking("second"),
      tool("c"),
    ];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("folds streaming thinking into the stack when tools continue", () => {
    const streaming: ChatMessage = {
      role: "Thinking",
      content: "",
      streaming: true,
    };
    const messages = [tool("a"), tool("b"), streaming, tool("c")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("folds leading thinking into a stack of 2 tools", () => {
    const messages = [thinking("warmup"), tool("a"), tool("b")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "toolStack", messages },
    ]);
  });

  it("keeps leading thinking outside a single tool", () => {
    const messages = [thinking("warmup"), tool("a")];
    expect(groupToolMessages(messages)).toEqual([
      { kind: "message", message: messages[0] },
      { kind: "tool", message: messages[1] },
    ]);
  });

  it("reuses toolStack object identity across live appends", () => {
    const two = [tool("a"), tool("b")];
    const first = groupToolMessages(two);
    expect(first).toHaveLength(1);
    const three = [...two, tool("c")];
    const second = groupToolMessages(three, undefined, first);
    expect(second).toHaveLength(1);
    expect(second[0]).toBe(first[0]);
    if (second[0]?.kind === "toolStack") {
      expect(second[0].messages).toEqual(three);
    }
  });
});
