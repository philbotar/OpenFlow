// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import type { Node, RunReport } from "../../lib/types";
import { PostRunSuggestions } from "./PostRunSuggestions";

const nodes = [{ id: "builder", label: "Builder" }] as Node[];

describe("PostRunSuggestions", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  function renderReport(report: RunReport) {
    container = document.createElement("div");
    document.body.appendChild(container);
    dispose = render(
      () => <PostRunSuggestions report={report} nodes={nodes} />,
      container,
    );
  }

  it("renders evidence-backed suggestions with their target node", () => {
    renderReport({
      workflow_id: "workflow",
      outputs: [],
      suggestions: [
        {
          id: "suggestion-1",
          category: "prompt",
          targetNodeId: "builder",
          title: "Require verification",
          evidence: "Builder claimed success without running tests.",
          recommendation: "Add a focused test command to the task prompt.",
        },
      ],
    });

    expect(container.textContent).toContain("Suggestions");
    expect(container.textContent).toContain("Builder");
    expect(container.textContent).toContain("Require verification");
    expect(container.textContent).toContain("claimed success without running tests");
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
