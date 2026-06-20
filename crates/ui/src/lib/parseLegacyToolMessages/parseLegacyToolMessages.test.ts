import { describe, expect, test } from "vitest";
import type { ChatMessage } from "../types";
import {
  groupLegacyToolMessages,
  isLegacyToolGroup,
  isProviderThinkingMessage,
} from ".";

describe("groupLegacyToolMessages", () => {
  test("groups request, running, and result lines for the same tool", () => {
    const messages: ChatMessage[] = [
      {
        role: "Thinking",
        content: "Tool request: read\nArguments:\n{\n  \"path\": \"README.md\"\n}",
      },
      { role: "Thinking", content: "Running tool: read" },
      { role: "Thinking", content: "Tool result: read\n¶README.md\n1:# OpenFlow" },
    ];

    const items = groupLegacyToolMessages(messages);
    expect(items).toHaveLength(1);
    expect(isLegacyToolGroup(items[0])).toBe(true);
    if (!isLegacyToolGroup(items[0])) return;

    expect(items[0].toolName).toBe("read");
    expect(items[0].status).toBe("completed");
    expect(items[0].output).toBe("¶README.md\n1:# OpenFlow");
    expect(items[0].argumentsText).toContain("\"path\": \"README.md\"");
  });

  test("passes through tool marker messages unchanged", () => {
    const messages: ChatMessage[] = [
      { role: "Thinking", content: "", toolCallId: "call-1" },
      { role: "System", content: "Approval required for tool 'read'." },
    ];

    const items = groupLegacyToolMessages(messages);
    expect(items).toHaveLength(2);
    expect(isLegacyToolGroup(items[0])).toBe(false);
    if (isLegacyToolGroup(items[0])) return;
    expect(items[0].toolCallId).toBe("call-1");
  });

  test("detects provider reasoning vs legacy tool thinking lines", () => {
    expect(
      isProviderThinkingMessage({
        role: "Thinking",
        content: "Let me work through the dependencies first.",
      }),
    ).toBe(true);
    expect(
      isProviderThinkingMessage({
        role: "Thinking",
        content: "Tool request: read\nArguments:\n{}",
      }),
    ).toBe(false);
    expect(
      isProviderThinkingMessage({
        role: "Thinking",
        content: "",
        toolCallId: "call-1",
      }),
    ).toBe(false);
  });

  test("leaves unrelated chat messages untouched", () => {
    const messages: ChatMessage[] = [
      { role: "Assistant", content: "Hello" },
      { role: "Thinking", content: "Tool request: read\nArguments:\n{}" },
      { role: "Thinking", content: "Tool result: read\ndone" },
    ];

    const items = groupLegacyToolMessages(messages);
    expect(items).toHaveLength(2);
    expect(isLegacyToolGroup(items[0])).toBe(false);
    if (isLegacyToolGroup(items[0])) return;
    expect(items[0].content).toBe("Hello");
    expect(isLegacyToolGroup(items[1])).toBe(true);
  });
});
