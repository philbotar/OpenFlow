// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { TextSelect } from "./TextSelect";

describe("TextSelect", () => {
  let container: HTMLDivElement;
  let dispose: () => void;

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  test("opens below trigger and selects a value", () => {
    const [value, setValue] = createSignal("write");
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <TextSelect
          value={value()}
          options={[
            { value: "read_only", label: "Read only" },
            { value: "write", label: "Read auto-approve, write prompt" },
          ]}
          onChange={(event) => setValue(event.currentTarget.value)}
        />
      ),
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    trigger.click();

    expect(container.querySelector(".text-select-menu")).not.toBeNull();

    const option = [...container.querySelectorAll(".text-select-option")].find(
      (element) => element.textContent === "Read only",
    ) as HTMLButtonElement;
    option.click();

    expect(value()).toBe("read_only");
    expect(container.querySelector(".text-select-menu")).toBeNull();
  });
});
