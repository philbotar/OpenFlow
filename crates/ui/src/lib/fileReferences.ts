import type { ProjectFileReferenceContent } from "./types";

export interface ActiveFileReferenceToken {
  query: string;
  replaceStart: number;
  replaceEnd: number;
}

const BRACED_FILE_REFERENCE_PATTERN = /(?:^|\s)(@\{([^}\n]*)$)/;
const PLAIN_FILE_REFERENCE_PATTERN = /(?:^|\s)(@([^\s{}]*)$)/;
const COMPLETE_FILE_REFERENCE_PATTERN = /@\{([^}\n]+)\}/g;

export function getActiveFileReferenceToken(
  input: string,
  caret: number,
): ActiveFileReferenceToken | null {
  const safeCaret = Math.max(0, Math.min(caret, input.length));
  const beforeCaret = input.slice(0, safeCaret);

  const braced = beforeCaret.match(BRACED_FILE_REFERENCE_PATTERN);
  if (braced?.[1] !== undefined) {
    const token = braced[1];
    return {
      query: braced[2] ?? "",
      replaceStart: safeCaret - token.length,
      replaceEnd: safeCaret,
    };
  }

  const plain = beforeCaret.match(PLAIN_FILE_REFERENCE_PATTERN);
  if (!plain?.[1]) {
    return null;
  }
  const token = plain[1];
  return {
    query: plain[2] ?? "",
    replaceStart: safeCaret - token.length,
    replaceEnd: safeCaret,
  };
}

export function applyFileReferenceCompletion(
  input: string,
  replaceStart: number,
  replaceEnd: number,
  path: string,
): { value: string; caret: number } {
  const token = `@{${path}}`;
  const value = `${input.slice(0, replaceStart)}${token} ${input.slice(replaceEnd)}`;
  return {
    value,
    caret: replaceStart + token.length + 1,
  };
}

export function extractReferencedFilePaths(input: string): string[] {
  const seen = new Set<string>();
  const paths: string[] = [];
  for (const match of input.matchAll(COMPLETE_FILE_REFERENCE_PATTERN)) {
    const path = match[1]?.trim();
    if (!path || seen.has(path)) {
      continue;
    }
    seen.add(path);
    paths.push(path);
  }
  return paths;
}

export function formatSubmissionWithFileReferences(
  submittedText: string,
  references: readonly ProjectFileReferenceContent[],
): string {
  if (references.length === 0) {
    return submittedText;
  }

  const lines = ["User message:", submittedText, "", "Referenced files:"];
  for (const reference of references) {
    lines.push("");
    lines.push(fileReferenceHeader(reference));
    lines.push("```text");
    lines.push(stripTrailingNewline(reference.content));
    lines.push("```");
  }
  return lines.join("\n");
}

function fileReferenceHeader(reference: ProjectFileReferenceContent): string {
  if (!reference.truncated) {
    return `File: ${reference.path}`;
  }
  return `File: ${reference.path} (truncated at 65536 bytes of ${reference.sizeBytes})`;
}

function stripTrailingNewline(value: string): string {
  return value.replace(/\n$/, "");
}
