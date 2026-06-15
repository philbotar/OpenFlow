import { describe, expect, test } from "vitest";
import type { ProjectFileReferenceContent } from "./types";
import {
  applyFileReferenceCompletion,
  extractReferencedFilePaths,
  formatSubmissionWithFileReferences,
  getActiveFileReferenceToken,
} from "./fileReferences";

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

  test("extracts unique braced file paths", () => {
    expect(
      extractReferencedFilePaths(
        "Read @{src/lib.rs} then @{README.md} and again @{src/lib.rs}",
      ),
    ).toEqual(["src/lib.rs", "README.md"]);
  });
});

describe("formatSubmissionWithFileReferences", () => {
  const refs: ProjectFileReferenceContent[] = [
    {
      path: "src/lib.rs",
      content: "pub fn value() -> u8 { 7 }\n",
      truncated: false,
      sizeBytes: 28,
    },
    {
      path: "src/large.rs",
      content: "pub fn partial() {}\n",
      truncated: true,
      sizeBytes: 70000,
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
        "Referenced files:",
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
      ].join("\n"),
    );
  });

  test("preserves structured skill submissions while appending references", () => {
    const skillRefs: ProjectFileReferenceContent[] = [
      {
        path: "README.md",
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
        "Referenced files:",
        "",
        "File: README.md",
        "```text",
        "# Project",
        "```",
      ].join("\n"),
    );
  });
});
