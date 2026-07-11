/** @vitest-environment jsdom */
import { render } from "solid-js/web";
import { describe, expect, it } from "vitest";
import { NodeCompletedBubble, normalizeDisplayValue } from "./NodeCompletedBubble";

describe("normalizeDisplayValue", () => {
  it("unwraps lone $text objects to the string", () => {
    expect(normalizeDisplayValue({ $text: "hello" })).toBe("hello");
    expect(normalizeDisplayValue([{ $text: "a" }, { $text: "b" }])).toEqual(["a", "b"]);
  });

  it("flattens nested item chains into a list and strips leaked XML tags", () => {
    expect(
      normalizeDisplayValue({
        integrationTests: [
          {
            $text: "[FEATURE] happy path.</item>",
            item: {
              $text: "Request flows through middleware.</item>",
              item: { $text: "Route -> service round-trip.</item>" },
            },
          },
        ],
      }),
    ).toEqual({
      integrationTests: [
        "[FEATURE] happy path.",
        "Request flows through middleware.",
        "Route -> service round-trip.",
      ],
    });
  });
});

describe("NodeCompletedBubble", () => {
  it("renders collapsible attributes from JSON", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(
      () => (
        <NodeCompletedBubble summary={JSON.stringify({ summary: "hello", ok: true })} />
      ),
      container,
    );

    expect(container.querySelectorAll(".node-completed-attr-key").length).toBe(2);
    expect(container.textContent).toContain("summary");
    expect(container.textContent).toContain("hello");

    const chevron = container.querySelector<HTMLButtonElement>(
      ".node-completed-attr-row .node-completed-chevron",
    );
    expect(chevron).not.toBeNull();
    chevron!.click();
    expect(container.textContent).toContain("…");
    expect(container.textContent).not.toContain("hello");

    dispose();
    container.remove();
  });

  it("shows flattened item lists without $text or nested item keys", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(
      () => (
        <NodeCompletedBubble
          summary={JSON.stringify({
            acceptanceCriteria: [
              {
                $text: "[FEATURE] ship it</item>",
                item: { $text: "Happy-path works</item>" },
              },
            ],
          })}
        />
      ),
      container,
    );

    expect(container.textContent).not.toContain("$text");
    expect(container.textContent).not.toContain("</item>");
    expect(container.textContent).toContain("[FEATURE] ship it");
    expect(container.textContent).toContain("Happy-path works");
    expect(container.textContent).toContain("acceptanceCriteria");
    expect(container.textContent).not.toMatch(/\bitem\b/);
    // Only the array field is collapsible — list rows have no chevrons.
    expect(container.querySelectorAll(".node-completed-chevron").length).toBe(1);
    expect(container.querySelectorAll(".node-completed-list-item").length).toBe(2);

    dispose();
    container.remove();
  });

  it("falls back to indented text for non-JSON", () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(
      () => <NodeCompletedBubble summary="summary: Done." />,
      container,
    );

    expect(container.querySelector(".node-completed-summary")?.textContent).toBe(
      "summary: Done.",
    );
    dispose();
    container.remove();
  });
});
