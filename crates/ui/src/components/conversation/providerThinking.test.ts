import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../../lib/types";
import { isProviderThinkingMessage } from "./providerThinking";

function thinking(content: string, extra: Partial<ChatMessage> = {}): ChatMessage {
  return { role: "thinking", content, ...extra };
}

describe("isProviderThinkingMessage", () => {
  it("accepts provider reasoning on thinking roles", () => {
    expect(isProviderThinkingMessage(thinking("Planning next step"))).toBe(true);
    expect(isProviderThinkingMessage({ role: "Thinking", content: "Hmm…" })).toBe(true);
  });

  it("rejects non-thinking roles", () => {
    expect(isProviderThinkingMessage({ role: "assistant", content: "Done" })).toBe(false);
    expect(isProviderThinkingMessage({ role: "user", content: "Go" })).toBe(false);
  });

  it("rejects thinking messages tied to a tool call", () => {
    expect(isProviderThinkingMessage(thinking("tool preamble", { toolCallId: "call-1" }))).toBe(
      false,
    );
  });

  it("rejects legacy tool I/O lines that reuse the thinking role", () => {
    expect(isProviderThinkingMessage(thinking("Tool request: read path"))).toBe(false);
    expect(isProviderThinkingMessage(thinking("Running tool: bash"))).toBe(false);
    expect(isProviderThinkingMessage(thinking("Tool result: ok"))).toBe(false);
  });
});
