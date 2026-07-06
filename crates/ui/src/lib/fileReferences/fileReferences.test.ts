import { describe, expect, test } from "vitest";
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
  test("leaves submissions without paths unchanged", () => {
    expect(formatSubmissionWithFileReferences("hello", [])).toBe("hello");
  });

  test("appends referenced paths after the user message", () => {
    const result = formatSubmissionWithFileReferences("check these", [
      "src/main.rs",
      "src/components/",
    ]);
    expect(result).toBe(
      [
        "User message:",
        "check these",
        "",
        "Referenced paths (relative to the execution folder; use your read and search tools to inspect them):",
        "- src/main.rs",
        "- src/components/",
      ].join("\n"),
    );
  });
});
