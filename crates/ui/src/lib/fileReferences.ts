import type { ProjectFileReferenceContent } from "./types";

export interface ActiveFileReferenceToken {
  query: string;
  replaceStart: number;
  replaceEnd: number;
}

const BRACED_FILE_REFERENCE_PATTERN = /(?:^|\s)(@\{([^}\n]*)$)/;
const PLAIN_FILE_REFERENCE_PATTERN = /(?:^|\s)(@([^\s{}]*)$)/;
const COMPLETE_FILE_REFERENCE_PATTERN = /@\{([^}\n]+)\}/g;

export type ComposerDisplaySegment =
  | { kind: "text"; value: string }
  | { kind: "fileRef"; path: string; token: string }
  | { kind: "skillRef"; skillId: string; token: string };

export function parseComposerDisplaySegments(
  input: string,
  knownSkillIds?: ReadonlySet<string>,
): ComposerDisplaySegment[] {
  const segments: ComposerDisplaySegment[] = [];
  let pos = 0;

  const leadingWhitespace = input.match(/^\s*/)?.[0] ?? "";
  if (leadingWhitespace.length > 0) {
    segments.push({ kind: "text", value: leadingWhitespace });
    pos = leadingWhitespace.length;
  }

  while (pos < input.length) {
    const rest = input.slice(pos);
    if (!rest.startsWith("/")) {
      break;
    }

    const separatorIndex = rest.search(/\s/);
    const token = separatorIndex === -1 ? rest : rest.slice(0, separatorIndex);
    const skillId = token.slice(1);
    if (skillId === "" || (knownSkillIds && !knownSkillIds.has(skillId))) {
      break;
    }

    segments.push({ kind: "skillRef", skillId, token });
    pos += token.length;

    if (separatorIndex === -1) {
      break;
    }

    const afterToken = input.slice(pos);
    const spaceMatch = afterToken.match(/^(\s+)/);
    if (!spaceMatch) {
      break;
    }

    const afterSpace = afterToken.slice(spaceMatch[1]!.length);
    if (afterSpace.startsWith("/")) {
      segments.push({ kind: "text", value: spaceMatch[1]! });
      pos += spaceMatch[1]!.length;
      continue;
    }

    break;
  }

  const remainder = input.slice(pos);
  let lastIndex = 0;

  for (const match of remainder.matchAll(COMPLETE_FILE_REFERENCE_PATTERN)) {
    const start = match.index ?? 0;
    if (start > lastIndex) {
      segments.push({ kind: "text", value: remainder.slice(lastIndex, start) });
    }
    const path = match[1]?.trim();
    if (path) {
      segments.push({ kind: "fileRef", path, token: match[0] });
    }
    lastIndex = start + match[0].length;
  }

  if (lastIndex < remainder.length) {
    segments.push({ kind: "text", value: remainder.slice(lastIndex) });
  }

  return segments;
}

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

  const lines = ["User message:", submittedText, "", "Referenced context:"];
  for (const reference of references) {
    lines.push("");
    lines.push(fileReferenceHeader(reference));
    if (reference.kind === "directory") {
      lines.push(stripTrailingNewline(reference.content));
    } else {
      lines.push("```text");
      lines.push(stripTrailingNewline(reference.content));
      lines.push("```");
    }
  }
  return lines.join("\n");
}

function fileReferenceHeader(reference: ProjectFileReferenceContent): string {
  if (reference.kind === "directory") {
    return reference.truncated
      ? `Directory: ${reference.path} (truncated)`
      : `Directory: ${reference.path}`;
  }
  if (!reference.truncated) {
    return `File: ${reference.path}`;
  }
  return `File: ${reference.path} (truncated at 65536 bytes of ${reference.sizeBytes})`;
}

function stripTrailingNewline(value: string): string {
  return value.replace(/\n$/, "");
}
