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
