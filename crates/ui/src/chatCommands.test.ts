import { describe, expect, test } from "vitest";
import { resolveChatSubmission } from "./chatCommands";

describe("resolveChatSubmission", () => {
  test("passes plain manual input through unchanged", () => {
    expect(resolveChatSubmission("  human approved ORCHID-91  ")).toEqual({
      bodyText: "human approved ORCHID-91",
      invokedSkills: [],
      submittedText: "human approved ORCHID-91",
    });
  });

  test("expands a leading skill command into a structured submission", () => {
    expect(resolveChatSubmission("/systematic-debugging Investigate ORCHID-91")).toEqual({
      bodyText: "Investigate ORCHID-91",
      invokedSkills: ["systematic-debugging"],
      submittedText: "Skill invocation:\n- systematic-debugging\n\nUser message:\nInvestigate ORCHID-91",
    });
  });

  test("collects multiple known skills and ignores duplicates", () => {
    expect(resolveChatSubmission("/brainstorming /documents /brainstorming Prepare the brief")).toEqual({
      bodyText: "Prepare the brief",
      invokedSkills: ["brainstorming", "documents"],
      submittedText: "Skill invocation:\n- brainstorming\n- documents\n\nUser message:\nPrepare the brief",
    });
  });

  test("keeps unknown slash-prefixed input as plain text", () => {
    expect(resolveChatSubmission("/not-a-skill keep literal slash text")).toEqual({
      bodyText: "/not-a-skill keep literal slash text",
      invokedSkills: [],
      submittedText: "/not-a-skill keep literal slash text",
    });
  });

  test("treats an unknown token after known skills as part of the user message", () => {
    expect(resolveChatSubmission("/browser /not-a-skill check the page")).toEqual({
      bodyText: "/not-a-skill check the page",
      invokedSkills: ["browser"],
      submittedText: "Skill invocation:\n- browser\n\nUser message:\n/not-a-skill check the page",
    });
  });

  test("supports command-only skill invocation", () => {
    expect(resolveChatSubmission("/requesting-code-review")).toEqual({
      bodyText: "",
      invokedSkills: ["requesting-code-review"],
      submittedText: "Skill invocation:\n- requesting-code-review",
    });
  });
});
