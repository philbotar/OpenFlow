// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { TodoChecklist, todoItemsFromArguments } from "./TodoChecklist";

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
