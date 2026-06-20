import { describe, expect, test } from "vitest";
import type { ProjectFileReferenceContent } from "../types";
import {
  applyFileReferenceCompletion,
  extractReferencedFilePaths,
  formatSubmissionWithFileReferences,
  getActiveFileReferenceToken,
  parseComposerDisplaySegments,
} from ".";

describe("file reference token helpers", () => {
  test("detects plain @ token before the caret", () => {
    expect(getActiveFileReferenceToken("Compare @src/mai", 16)).toEqual({
      query: "src/mai",
      replaceStart: 8,
      replaceEnd: 16,
    });
  });

  test("detects braced @ token before the caret", () => {
    expect(getActiveFileReferenceToken("Compare @{src/mai", 17)).toEqual({
      query: "src/mai",
      replaceStart: 8,
      replaceEnd: 17,
    });
  });

  test("ignores @ inside email addresses", () => {
    expect(getActiveFileReferenceToken("mail test@example.com", 21)).toBeNull();
  });

  test("applies braced completion", () => {
    expect(applyFileReferenceCompletion("Open @src/mai", 5, 13, "src/main.rs")).toEqual({
      value: "Open @{src/main.rs} ",
      caret: 20,
    });
  });

  test("keeps folder completions with a trailing slash", () => {
    expect(
      applyFileReferenceCompletion("Open @src/com", 5, 13, "src/components/"),
    ).toEqual({
      value: "Open @{src/components/} ",
      caret: 24,
    });
  });

  test("extracts unique braced file paths", () => {
    expect(
      extractReferencedFilePaths(
        "Read @{src/lib.rs} then @{README.md} and again @{src/lib.rs}",
      ),
    ).toEqual(["src/lib.rs", "README.md"]);
  });

  test("parses display segments with inline file chips", () => {
    expect(parseComposerDisplaySegments("Review @{crates/} please")).toEqual([
      { kind: "text", value: "Review " },
      { kind: "fileRef", path: "crates/", token: "@{crates/}" },
      { kind: "text", value: " please" },
    ]);
    expect(parseComposerDisplaySegments("Plain message")).toEqual([
      { kind: "text", value: "Plain message" },
    ]);
    expect(parseComposerDisplaySegments("")).toEqual([]);
  });

  test("parses leading skill tokens as inline chips", () => {
    const knownSkills = new Set(["brainstorming", "documents"]);

    expect(
      parseComposerDisplaySegments("/brainstorming Prepare the brief", knownSkills),
    ).toEqual([
      { kind: "skillRef", skillId: "brainstorming", token: "/brainstorming" },
      { kind: "text", value: " Prepare the brief" },
    ]);

    expect(
      parseComposerDisplaySegments(
        "/brainstorming /documents Review @{README.md}",
        knownSkills,
      ),
    ).toEqual([
      { kind: "skillRef", skillId: "brainstorming", token: "/brainstorming" },
      { kind: "text", value: " " },
      { kind: "skillRef", skillId: "documents", token: "/documents" },
      { kind: "text", value: " Review " },
      { kind: "fileRef", path: "README.md", token: "@{README.md}" },
    ]);

    expect(parseComposerDisplaySegments("/not-a-skill keep literal", knownSkills)).toEqual([
      { kind: "text", value: "/not-a-skill keep literal" },
    ]);
  });
});

describe("formatSubmissionWithFileReferences", () => {
  const refs: ProjectFileReferenceContent[] = [
    {
      path: "src/lib.rs",
      kind: "file",
      content: "pub fn value() -> u8 { 7 }\n",
      truncated: false,
      sizeBytes: 28,
    },
    {
      path: "src/large.rs",
      kind: "file",
      content: "pub fn partial() {}\n",
      truncated: true,
      sizeBytes: 70000,
    },
    {
      path: "src/components/",
      kind: "directory",
      content: [
        "Directory tree:",
        "src/components/",
        "- Button.tsx",
        "",
        "File: src/components/Button.tsx",
        "```text",
        "export function Button() {}",
        "```",
      ].join("\n"),
      truncated: false,
      sizeBytes: 27,
    },
  ];

  test("leaves submissions without files unchanged", () => {
    expect(formatSubmissionWithFileReferences("Plain message", [])).toBe("Plain message");
  });

  test("appends file contents after the user message", () => {
    expect(formatSubmissionWithFileReferences("Review @{src/lib.rs}", refs)).toBe(
      [
        "User message:",
        "Review @{src/lib.rs}",
        "",
        "Referenced context:",
        "",
        "File: src/lib.rs",
        "```text",
        "pub fn value() -> u8 { 7 }",
        "```",
        "",
        "File: src/large.rs (truncated at 65536 bytes of 70000)",
        "```text",
        "pub fn partial() {}",
        "```",
        "",
        "Directory: src/components/",
        "Directory tree:",
        "src/components/",
        "- Button.tsx",
        "",
        "File: src/components/Button.tsx",
        "```text",
        "export function Button() {}",
        "```",
      ].join("\n"),
    );
  });

  test("preserves structured skill submissions while appending references", () => {
    const skillRefs: ProjectFileReferenceContent[] = [
      {
        path: "README.md",
        kind: "file",
        content: "# Project\n",
        truncated: false,
        sizeBytes: 10,
      },
    ];

    expect(
      formatSubmissionWithFileReferences(
        "Skill invocation:\n- brainstorming\n\nUser message:\nReview @{README.md}",
        skillRefs,
      ),
    ).toBe(
      [
        "User message:",
        "Skill invocation:",
        "- brainstorming",
        "",
        "User message:",
        "Review @{README.md}",
        "",
        "Referenced context:",
        "",
        "File: README.md",
        "```text",
        "# Project",
        "```",
      ].join("\n"),
    );
  });
});
