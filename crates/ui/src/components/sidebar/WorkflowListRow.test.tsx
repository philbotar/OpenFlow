// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { WorkflowListRow } from "./WorkflowListRow";

describe("WorkflowListRow", () => {
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

  test("opens a menu and requests rename or deletion", () => {
    const onRename = vi.fn();
    const onDelete = vi.fn();
    dispose = render(
      () => (
        <WorkflowListRow
          title="Release flow"
          active={false}
          editing={false}
          onSelect={vi.fn()}
          onRename={onRename}
          onDelete={onDelete}
          editSlot={<input aria-label="Workflow name" />}
        />
      ),
      container,
    );

    const menuButton = container.querySelector(
      "button[aria-label='Workflow options for Release flow']",
    ) as HTMLButtonElement;
    menuButton.click();
    const menuItems = () =>
      Array.from(
        container.querySelectorAll<HTMLButtonElement>("[role='menuitem']"),
      );

    menuItems().find((button) => button.textContent === "Rename")!.click();
    expect(onRename).toHaveBeenCalledTimes(1);

    menuButton.click();
    menuItems()
      .find((button) => button.textContent === "Delete workflow")!
      .click();
    expect(onDelete).toHaveBeenCalledTimes(1);
  });
});
