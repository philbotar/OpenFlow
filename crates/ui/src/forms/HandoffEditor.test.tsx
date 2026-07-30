// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { HandoffEditor } from "./HandoffEditor";

describe("HandoffEditor", () => {
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

  test("projects Markdown template edits through its public callback", () => {
    const onHandoffChange = vi.fn();
    dispose = render(
      () => (
        <HandoffEditor
          handoff={{
            format: "markdown",
            template: "# Handoff\n\n## Summary\n",
          }}
          schemaJson="{}"
          onHandoffChange={onHandoffChange}
          onSchemaChange={vi.fn()}
          onApplySchema={vi.fn()}
        />
      ),
      container,
    );

    const template = container.querySelector(
      'textarea[aria-label="Markdown handoff template"]',
    ) as HTMLTextAreaElement;
    template.value = "# Result\n\n## Decision\n";
    template.dispatchEvent(new InputEvent("input", { bubbles: true }));

    expect(onHandoffChange).toHaveBeenCalledWith({
      format: "markdown",
      template: "# Result\n\n## Decision\n",
    });
    expect(container.textContent).not.toContain("JSON output schema");
  });

  test("switches to a JSON artifact through its public callback", () => {
    const onHandoffChange = vi.fn();
    dispose = render(
      () => (
        <HandoffEditor
          handoff={{
            format: "markdown",
            template: "# Handoff\n",
          }}
          schemaJson="{}"
          onHandoffChange={onHandoffChange}
          onSchemaChange={vi.fn()}
          onApplySchema={vi.fn()}
        />
      ),
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    trigger.click();
    const jsonOption = [...container.querySelectorAll(".text-select-option")].find(
      (element) => element.textContent === "JSON",
    ) as HTMLButtonElement;
    jsonOption.click();

    expect(onHandoffChange).toHaveBeenCalledWith({ format: "json" });
  });
});
