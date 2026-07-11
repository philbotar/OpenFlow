import type { ChatRole } from "../types";

/** Mirror of `engine::conversation::strip_tool_call_markup`. */

function consumeToolCallFenceBlock(content: string): number {
  const open = "```tool_call";
  if (!content.startsWith(open)) return 0;

  let consumed = open.length;
  const rest = content.slice(consumed);
  if (rest.startsWith("\r\n")) {
    consumed += 2;
  } else if (rest.startsWith("\n")) {
    consumed += 1;
  }

  const body = content.slice(consumed);
  const close = body.indexOf("```");
  return close >= 0 ? consumed + close + 3 : content.length;
}

function consumeToolCallXmlBlock(content: string): number {
  const open = "<tool_call";
  const close = "</tool_call>";
  if (!content.startsWith(open)) return 0;
  const end = content.indexOf(close);
  return end >= 0 ? end + close.length : content.length;
}

const TOOL_CALL_PREFIXES = ["```tool_call", "<tool_call"] as const;

/** Hide in-progress `<tool_call` / ` ```tool_call ` tokens while streaming. */
function stripTrailingPartialToolCallPrefix(content: string): string {
  for (const prefix of TOOL_CALL_PREFIXES) {
    for (let len = prefix.length - 1; len > 0; len -= 1) {
      const partial = prefix.slice(0, len);
      if (content.endsWith(partial)) {
        return content.slice(0, -partial.length);
      }
    }
  }
  return content;
}

export function stripToolCallMarkup(content: string): string {
  let result = "";
  let rest = content;

  while (rest.length > 0) {
    const xmlIndex = rest.indexOf("<tool_call");
    const fenceIndex = rest.indexOf("```tool_call");

    if (xmlIndex < 0 && fenceIndex < 0) {
      result += rest;
      break;
    }

    let start: number;
    let isXml: boolean;
    if (xmlIndex < 0) {
      start = fenceIndex;
      isXml = false;
    } else if (fenceIndex < 0) {
      start = xmlIndex;
      isXml = true;
    } else {
      start = xmlIndex <= fenceIndex ? xmlIndex : fenceIndex;
      isXml = xmlIndex <= fenceIndex;
    }

    result += rest.slice(0, start);
    const block = rest.slice(start);
    const consumed = isXml ? consumeToolCallXmlBlock(block) : consumeToolCallFenceBlock(block);
    if (consumed === 0) {
      result += rest;
      break;
    }
    rest = rest.slice(start + consumed);
  }

  return stripTrailingPartialToolCallPrefix(result).trim();
}

function shouldStripToolCallMarkup(role: ChatRole): boolean {
  return (
    role === "assistant" ||
    role === "Assistant" ||
    role === "thinking" ||
    role === "Thinking"
  );
}

const THINK_BLOCK = /<think\b[^>]*>([\s\S]*?)<\/think>/gi;
const THINK_OPEN = /<think\b[^>]*>/i;

/** Pull `<think>` bodies out of assistant text; remainder is visible prose. */
export function extractThinkContent(content: string): {
  thoughts: string;
  remainder: string;
} {
  const parts: string[] = [];
  let remainder = content.replace(THINK_BLOCK, (_match, body: string) => {
    const trimmed = String(body).trim();
    if (trimmed) parts.push(trimmed);
    return "";
  });
  const open = remainder.search(THINK_OPEN);
  if (open >= 0) {
    const afterOpen = remainder.slice(open).replace(THINK_OPEN, "");
    if (afterOpen.trim()) parts.push(afterOpen.trim());
    remainder = remainder.slice(0, open);
  }
  return {
    thoughts: parts.join("\n\n"),
    remainder: remainder.trim(),
  };
}

/** Remove provider `<think>…</think>` blocks (and an unclosed trailing open tag). */
export function stripThinkMarkup(content: string): string {
  return extractThinkContent(content).remainder;
}

function shouldStripThinkMarkup(role: ChatRole): boolean {
  return shouldStripToolCallMarkup(role);
}

/** Chat display text: strip tool-call echo + think blocks for assistant/thinking. */
export function displayChatContent(role: ChatRole, content: string): string {
  let text = content;
  if (shouldStripToolCallMarkup(role)) {
    text = stripToolCallMarkup(text);
  }
  if (shouldStripThinkMarkup(role)) {
    text = stripThinkMarkup(text);
  }
  return text.trim();
}
