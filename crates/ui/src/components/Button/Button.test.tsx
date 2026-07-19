// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  let container: HTMLDivElement;
  let dispose: () => void;

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  test("renders primary variant classes", () => {
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <Button variant="primary">Save</Button>, container);

    const button = container.querySelector("button");
    expect(button?.className).toContain("primary-button");
    expect(button?.textContent).toBe("Save");
  });

  test("renders secondary ghost small modifiers", () => {
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <Button variant="secondary" ghost size="small" class="providers-icon-button">
          Remove
        </Button>
      ),
      container,
    );

    const button = container.querySelector("button");
    expect(button?.className).toBe("secondary-button small ghost providers-icon-button");
  });
});
