// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { TodoChecklist, todoItemsFromArguments } from "./TodoChecklist";

const indexCss = readFileSync("src/styles/index.css", "utf8");
const chatCss = readFileSync("src/styles/chat.css", "utf8");

describe("TodoChecklist", () => {
  let dispose: (() => void) | undefined;

  afterEach(() => {
    dispose?.();
    document.body.replaceChildren();
  });

  it("parses tool arguments and renders phase states", () => {
    const items = todoItemsFromArguments({
      todos: [
        { content: "Trace behavior", status: "completed" },
        { content: "Implement checklist", status: "in_progress" },
        { content: "Verify gates", status: "pending" },
      ],
    });
    expect(items).not.toBeNull();

    const container = document.createElement("div");
    document.body.appendChild(container);
    dispose = render(
      () => <TodoChecklist items={items!} toolStatus="completed" />,
      container,
    );

    expect(container.textContent).toContain("Progress1/3");
    expect(container.querySelectorAll(".todo-checklist-item")).toHaveLength(3);
    expect(
      container
        .querySelector(".todo-checklist-item.status-in_progress")
        ?.getAttribute("aria-current"),
    ).toBe("step");
  });

  it("colors only the completed tick with the success color", () => {
    const style = document.createElement("style");
    style.textContent = `${indexCss}\n${chatCss}`
      .split("var(--success)")
      .join("rgb(0, 128, 0)");
    document.head.append(style);

    const container = document.createElement("div");
    document.body.appendChild(container);
    dispose = render(
      () => (
        <TodoChecklist
          items={[{ content: "Trace behavior", status: "completed" }]}
          toolStatus="completed"
        />
      ),
      container,
    );

    try {
      const row = container.querySelector<HTMLElement>(".status-completed");
      const marker = row?.querySelector<HTMLElement>(".todo-checklist-marker");
      const tick = marker?.querySelector("svg");

      expect(row).not.toBeNull();
      expect(marker).not.toBeNull();
      expect(getComputedStyle(row!).backgroundColor).toBe("rgba(0, 0, 0, 0)");
      expect(getComputedStyle(marker!).backgroundColor).toBe("rgba(0, 0, 0, 0)");
      expect(getComputedStyle(marker!).color).toBe("rgb(0, 128, 0)");
      expect(tick?.getAttribute("stroke")).toBe("currentColor");
    } finally {
      style.remove();
    }
  });

  it("rejects malformed todo arguments", () => {
    expect(
      todoItemsFromArguments({
        todos: [{ content: "", status: "in_progress" }],
      }),
    ).toBeNull();
    expect(
      todoItemsFromArguments({
        todos: [{ content: "Unknown", status: "started" }],
      }),
    ).toBeNull();
  });
});
