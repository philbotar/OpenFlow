// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Node, PostRunSuggestion, RunReport } from "../../lib/types";
import { PostRunSuggestions } from "./PostRunSuggestions";

const nodes = [{ id: "builder", label: "Builder" }] as Node[];
const suggestion: PostRunSuggestion = {
  id: "suggestion-1",
  category: "prompt",
  targetNodeId: "builder",
  title: "Require verification",
  evidence: "Builder claimed success without running tests.",
  recommendation: "Add a focused test command to the task prompt.",
};

describe("PostRunSuggestions", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  function renderReport(
    report: RunReport,
    onApply = vi.fn<(suggestion: PostRunSuggestion) => void>(),
  ) {
    container = document.createElement("div");
    document.body.appendChild(container);
    dispose = render(
      () => <PostRunSuggestions report={report} nodes={nodes} onApply={onApply} />,
      container,
    );
  }

  it("consolidates each evidence-backed suggestion into one paragraph", () => {
    renderReport({
      workflow_id: "workflow",
      outputs: [],
      suggestions: [suggestion],
    });

    expect(container.textContent).toContain("Suggestions");
    expect(container.textContent).toContain("Builder");
    expect(container.textContent).toContain("Require verification");
    expect(container.textContent).toContain("claimed success without running tests");
    expect(container.querySelectorAll(".post-run-suggestion p")).toHaveLength(1);
  });

  it("applies the selected suggestion through the public callback", () => {
    const onApply = vi.fn<(suggestion: PostRunSuggestion) => void>();
    renderReport(
      {
        workflow_id: "workflow",
        outputs: [],
        suggestions: [suggestion],
      },
      onApply,
    );

    const button = container.querySelector<HTMLButtonElement>(
      'button[aria-label="Apply Require verification with AI"]',
    );
    expect(button).not.toBeNull();

    button?.click();

    expect(onApply).toHaveBeenCalledWith(suggestion);
  });

  it("shows reviewer failure without changing the run result", () => {
    renderReport({
      workflow_id: "workflow",
      outputs: [],
      suggestions_error: "Reviewer request failed.",
    });

    expect(container.textContent).toContain("Suggestions unavailable");
    expect(container.textContent).toContain("Reviewer request failed.");
  });
});
