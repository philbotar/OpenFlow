// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { TextSelect } from "./TextSelect";

describe("TextSelect", () => {
  let container: HTMLDivElement;
  let scrollHost: HTMLDivElement | undefined;
  let dispose: () => void;

  afterEach(() => {
    dispose?.();
    container?.remove();
    scrollHost?.remove();
    scrollHost = undefined;
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

  test("keeps menu open when scrolling inside the listbox", () => {
    const manyOptions = Array.from({ length: 12 }, (_, index) => ({
      value: `option-${index}`,
      label: `Option ${index}`,
    }));
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => <TextSelect value="option-0" options={manyOptions} />,
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    trigger.click();

    const menu = container.querySelector(".text-select-menu") as HTMLUListElement;
    expect(menu).not.toBeNull();
    menu.dispatchEvent(new Event("scroll", { bubbles: false }));

    expect(container.querySelector(".text-select-menu")).not.toBeNull();
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
  });

  test("closes menu when an ancestor outside the root scrolls", () => {
    container = document.createElement("div");
    scrollHost = document.createElement("div");
    scrollHost.style.height = "100px";
    scrollHost.style.overflow = "auto";
    scrollHost.append(container);
    document.body.append(scrollHost);
    dispose = render(
      () => (
        <TextSelect
          value="a"
          options={[
            { value: "a", label: "A" },
            { value: "b", label: "B" },
          ]}
        />
      ),
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    trigger.click();
    expect(container.querySelector(".text-select-menu")).not.toBeNull();

    scrollHost.dispatchEvent(new Event("scroll", { bubbles: false }));

    expect(container.querySelector(".text-select-menu")).toBeNull();
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
  });

  test("opens above trigger when menuPlacement is above", () => {
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <TextSelect
          menuPlacement="above"
          value="write"
          options={[{ value: "write", label: "Write" }]}
        />
      ),
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    trigger.getBoundingClientRect = () =>
      ({
        top: 400,
        bottom: 424,
        left: 16,
        width: 120,
        right: 136,
        height: 24,
        x: 16,
        y: 400,
        toJSON: () => ({}),
      }) as DOMRect;
    trigger.click();

    const menu = container.querySelector(".text-select-menu") as HTMLUListElement;
    expect(menu.classList.contains("text-select-menu--above")).toBe(true);
    expect(menu.style.transform).toBe("translateY(-100%)");
    expect(menu.style.top).toBe("396px");
  });
});
