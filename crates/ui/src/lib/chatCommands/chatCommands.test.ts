import { describe, expect, test } from "vitest";
import type { SkillSummary } from "../types";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
  resolveChatSubmission,
} from ".";

const FIXTURE_SKILL_IDS = new Set([
  "brainstorming",
  "test-driven-development",
  "systematic-debugging",
  "writing-plans",
  "executing-plans",
  "verification-before-completion",
  "requesting-code-review",
  "receiving-code-review",
  "github:gh-fix-ci",
  "github:gh-address-comments",
  "browser",
  "documents",
]);

describe("resolveChatSubmission", () => {
  test("passes plain manual input through unchanged", () => {
    expect(resolveChatSubmission("  human approved ORCHID-91  ", FIXTURE_SKILL_IDS)).toEqual({
      bodyText: "human approved ORCHID-91",
      invokedSkills: [],
      submittedText: "human approved ORCHID-91",
    });
  });

  test("expands a leading skill command into a structured submission", () => {
    expect(
      resolveChatSubmission("/systematic-debugging Investigate ORCHID-91", FIXTURE_SKILL_IDS),
    ).toEqual({
      bodyText: "Investigate ORCHID-91",
      invokedSkills: ["systematic-debugging"],
      submittedText: "Skill invocation:\n- systematic-debugging\n\nUser message:\nInvestigate ORCHID-91",
    });
  });

  test("collects multiple known skills and ignores duplicates", () => {
    expect(
      resolveChatSubmission(
        "/brainstorming /documents /brainstorming Prepare the brief",
        FIXTURE_SKILL_IDS,
      ),
    ).toEqual({
      bodyText: "Prepare the brief",
      invokedSkills: ["brainstorming", "documents"],
      submittedText: "Skill invocation:\n- brainstorming\n- documents\n\nUser message:\nPrepare the brief",
    });
  });

  test("keeps unknown slash-prefixed input as plain text", () => {
    expect(resolveChatSubmission("/not-a-skill keep literal slash text", FIXTURE_SKILL_IDS)).toEqual({
      bodyText: "/not-a-skill keep literal slash text",
      invokedSkills: [],
      submittedText: "/not-a-skill keep literal slash text",
    });
  });

  test("treats an unknown token after known skills as part of the user message", () => {
    expect(resolveChatSubmission("/browser /not-a-skill check the page", FIXTURE_SKILL_IDS)).toEqual({
      bodyText: "/not-a-skill check the page",
      invokedSkills: ["browser"],
      submittedText: "Skill invocation:\n- browser\n\nUser message:\n/not-a-skill check the page",
    });
  });

  test("supports command-only skill invocation", () => {
    expect(resolveChatSubmission("/requesting-code-review", FIXTURE_SKILL_IDS)).toEqual({
      bodyText: "",
      invokedSkills: ["requesting-code-review"],
      submittedText: "Skill invocation:\n- requesting-code-review",
    });
  });
});

const FIXTURE_SKILLS: SkillSummary[] = [
  { id: "brainstorming", name: "Brainstorming", description: "Explore ideas." },
  { id: "systematic-debugging", name: "Systematic Debugging", description: "Debug bugs." },
  { id: "browser", name: "Browser", description: "Use the browser." },
];

describe("slash command combobox helpers", () => {
  test("detects the slash token before the caret", () => {
    expect(getActiveSlashToken("/systematic-debugging Investigate", 21)).toEqual({
      query: "systematic-debugging",
      replaceStart: 0,
      replaceEnd: 21,
    });
    expect(getActiveSlashToken("/brainstorming /bro", 19)).toEqual({
      query: "bro",
      replaceStart: 15,
      replaceEnd: 19,
    });
    expect(getActiveSlashToken("plain text", 10)).toBeNull();
  });

  test("filters skills by id and name prefix", () => {
    expect(matchSkillsForSlashQuery(FIXTURE_SKILLS, "sys").map((skill) => skill.id)).toEqual([
      "systematic-debugging",
    ]);
    expect(matchSkillsForSlashQuery(FIXTURE_SKILLS, "brain").map((skill) => skill.id)).toEqual([
      "brainstorming",
    ]);
    expect(matchSkillsForSlashQuery(FIXTURE_SKILLS, "bro").map((skill) => skill.id)).toEqual([
      "browser",
    ]);
  });

  test("applies a selected slash token completion", () => {
    expect(
      applySlashTokenCompletion("/sys", 0, 4, "systematic-debugging"),
    ).toEqual({
      value: "/systematic-debugging ",
      caret: 22,
    });
  });
});
