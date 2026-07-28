// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import type { SkillSummary } from "@/lib/types";
import { SkillPromptTextarea } from "./SkillPromptTextarea";

const skills: SkillSummary[] = [
  {
    id: "tdd",
    name: "Test-driven development",
    description: "Build one red-green slice at a time.",
    path: "/skills/tdd/SKILL.md",
  },
];

describe("SkillPromptTextarea", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  test("completes an installed skill at the start of a task prompt", () => {
    const [value, setValue] = createSignal("");
    dispose = render(
      () => (
        <SkillPromptTextarea
          value={value()}
          onInput={setValue}
          skills={skills}
          rows={3}
        />
      ),
      container,
    );
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;

    textarea.value = "/td";
    textarea.setSelectionRange(3, 3);
    textarea.dispatchEvent(new InputEvent("input", { bubbles: true }));

    const option = container.querySelector(".skill-command-option") as HTMLButtonElement;
    expect(option.textContent).toContain("/tdd");
    option.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));

    expect(value()).toBe("/tdd ");
  });

  test("lists and completes installed skills after task prose", () => {
    const [value, setValue] = createSignal("");
    dispose = render(
      () => (
        <SkillPromptTextarea
          value={value()}
          onInput={setValue}
          skills={skills}
          rows={3}
        />
      ),
      container,
    );
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;

    textarea.value = "Review this /";
    textarea.setSelectionRange(13, 13);
    textarea.dispatchEvent(new InputEvent("input", { bubbles: true }));

    const option = container.querySelector(".skill-command-option") as HTMLButtonElement;
    expect(option.textContent).toContain("/tdd");
    option.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));

    expect(value()).toBe("Review this /tdd ");
  });

  test("renders the normal chat skill bubble for installed task prompt tokens", () => {
    const [value, setValue] = createSignal("Review this with /tdd");
    dispose = render(
      () => (
        <SkillPromptTextarea
          value={value()}
          onInput={setValue}
          skills={skills}
          rows={3}
        />
      ),
      container,
    );

    const preview = container.querySelector(".skill-description-preview");
    expect(preview?.textContent).toContain("/tdd");
    expect(preview?.textContent).toContain("Test-driven development");
    expect(preview?.textContent).toContain("Build one red-green slice at a time.");
  });
});
