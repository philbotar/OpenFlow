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

  test("returns focus to the chat menu trigger after keyboard dismissal", () => {
    dispose = render(() => <ChatHistoryRow title="Notes" active={false}
      onSelect={vi.fn()} onDelete={vi.fn()} />, container);
    const trigger = container.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!;
    trigger.focus();
    trigger.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true }));
    expect(document.activeElement?.getAttribute("role")).toBe("menuitem");
    document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    expect(container.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
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
