// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { ChatHistoryRow } from "./ChatHistoryRow";

describe("ChatHistoryRow", () => {
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

  test("opens a menu and requests deletion for its chat", () => {
    const onDelete = vi.fn();
    dispose = render(
      () => (
        <ChatHistoryRow
          title="Project notes"
          active={false}
          onSelect={vi.fn()}
          onDelete={onDelete}
        />
      ),
      container,
    );

    const menuButton = container.querySelector(
      "button[aria-label='Chat options for Project notes']",
    ) as HTMLButtonElement;
    menuButton.click();

    const deleteButton = Array.from(
      container.querySelectorAll<HTMLButtonElement>("[role='menuitem']"),
    ).find((button) => button.textContent === "Delete chat");
    expect(deleteButton).toBeDefined();

    deleteButton!.click();
    expect(onDelete).toHaveBeenCalledTimes(1);
  });
});
