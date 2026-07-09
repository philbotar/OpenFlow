import { describe, expect, test } from "vitest";
import type { AgentStatus, ChatMessage, NodeId } from "../types";
import {
  isChatNavigatedToNode,
  type ChatLayoutProjection,
  type TranscriptSegment,
} from "./chatLayout";

function segment(nodeId: NodeId, status: AgentStatus = "idle"): TranscriptSegment {
  return { nodeId, label: nodeId, messages: [] as ChatMessage[], status };
}

describe("isChatNavigatedToNode", () => {
  test("detects settled filter already active", () => {
    const layout: ChatLayoutProjection = {
      settled: [segment("node-a")],
      live: [],
    };
    expect(isChatNavigatedToNode(layout, "node-a", "node-a", null)).toBe(true);
    expect(isChatNavigatedToNode(layout, "node-a", null, null)).toBe(false);
  });

  test("detects live pick already active", () => {
    const layout: ChatLayoutProjection = {
      settled: [],
      live: [segment("node-b", "awaiting_tool_approval")],
    };
    expect(isChatNavigatedToNode(layout, "node-b", null, "node-b")).toBe(true);
    expect(isChatNavigatedToNode(layout, "node-b", "node-b", null)).toBe(false);
  });
});
