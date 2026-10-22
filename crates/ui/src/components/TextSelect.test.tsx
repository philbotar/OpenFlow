// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test, vi } from "vitest";
import { TextSelect } from "./TextSelect";

describe("TextSelect", () => {
  const initialInnerHeight = window.innerHeight;
  const initialInnerWidth = window.innerWidth;
  let container: HTMLDivElement;
  let scrollHost: HTMLDivElement | undefined;
  let dispose: () => void;

  test.each([1, 1.5])("keeps a menu inside the right viewport edge at zoom %s", async (scale) => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 300 });
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <TextSelect value="a" options={[{ value: "a", label: "A" }]} />, container);
    const trigger = container.querySelector<HTMLButtonElement>(".text-select-trigger")!;
    Object.defineProperty(trigger, "offsetWidth", { value: 100 });
    trigger.getBoundingClientRect = () => ({ left: 250, right: 250 + 100 * scale, top: 40, bottom: 60, width: 100 * scale, height: 20 }) as DOMRect;
    trigger.click();
    await Promise.resolve();
    const menu = container.querySelector<HTMLElement>(".text-select-menu")!;
    expect(parseFloat(menu.style.left)).toBeGreaterThanOrEqual(0);
    expect((parseFloat(menu.style.left) + parseFloat(menu.style.width)) * scale).toBeLessThanOrEqual(300);
  });

  test.each([false, true])("supports keyboard navigation and Escape (portal=%s)", async (portalMenu) => {
    const onChange = vi.fn();
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <TextSelect value="b" portalMenu={portalMenu}
      options={[{ value: "a", label: "A" }, { value: "b", label: "B" }, { value: "c", label: "C" }]}
      onChange={onChange} />, container);
    const trigger = container.querySelector<HTMLButtonElement>(".text-select-trigger")!;
    const key = (name: string) => document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: name, bubbles: true, cancelable: true }));
    trigger.focus();
    key("ArrowDown");
    await Promise.resolve();
    expect(document.activeElement?.textContent).toBe("B");
    key("ArrowDown");
    expect(document.activeElement?.textContent).toBe("C");
    key("Home");
    expect(document.activeElement?.textContent).toBe("A");
    key("End");
    expect(document.activeElement?.textContent).toBe("C");
    key("Escape");
    expect(document.body.querySelector(".text-select-menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
    expect(onChange).not.toHaveBeenCalled();
    key("ArrowUp");
    await Promise.resolve();
    key("Home");
    (document.activeElement as HTMLButtonElement).click();
    expect(onChange).toHaveBeenCalledWith({ currentTarget: { value: "a" } });
    expect(document.activeElement).toBe(trigger);
  });

  test("closes an open select when disabled and rejects a stale option click", () => {
    const [disabled, setDisabled] = createSignal(false);
    const onChange = vi.fn();
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <TextSelect value="a" disabled={disabled()}
      options={[{ value: "a", label: "A" }]} onChange={onChange} />, container);
    container.querySelector<HTMLButtonElement>(".text-select-trigger")!.click();
    const option = container.querySelector<HTMLButtonElement>(".text-select-option")!;
    setDisabled(true);
    expect(container.querySelector(".text-select-menu")).toBeNull();
    option.click();
    expect(onChange).not.toHaveBeenCalled();
  });

  afterEach(() => {
    dispose?.();
    container?.remove();
    scrollHost?.remove();
    scrollHost = undefined;
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: initialInnerHeight,
    });
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: initialInnerWidth,
    });
    vi.useRealTimers();
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

  test("opens above automatically when the menu does not fit below", async () => {
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 450,
    });
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <TextSelect
          value=""
          options={[
            { value: "", label: "None" },
            { value: "fast", label: "Fast" },
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
          ]}
        />
      ),
      container,
    );

    const trigger = container.querySelector(".text-select-trigger") as HTMLButtonElement;
    Object.defineProperty(trigger, "offsetWidth", {
      configurable: true,
      value: 320,
    });
    trigger.getBoundingClientRect = () =>
      ({
        top: 400,
        bottom: 424,
        left: 16,
        width: 320,
        right: 336,
        height: 24,
        x: 16,
        y: 400,
        toJSON: () => ({}),
      }) as DOMRect;
    trigger.click();

    const menu = container.querySelector(".text-select-menu") as HTMLUListElement;
    menu.getBoundingClientRect = () =>
      ({
        top: 0,
        bottom: 168,
        left: 16,
        width: 320,
        right: 336,
        height: 168,
        x: 16,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect;
    await Promise.resolve();

    expect(menu.classList.contains("text-select-menu--above")).toBe(true);
    expect(menu.style.transform).toBe("translateY(-100%)");
    expect(menu.style.top).toBe("396px");
  });

  test("opens a horizontal menu on hover and keeps it open across the gap", async () => {
    vi.useFakeTimers();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 700,
    });
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <TextSelect
          menuPlacement="horizontal"
          openOnHover
          portalMenu
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
    Object.defineProperty(trigger, "offsetWidth", {
      configurable: true,
      value: 120,
    });
    trigger.getBoundingClientRect = () =>
      ({
        top: 100,
        bottom: 130,
        left: 500,
        right: 620,
        width: 120,
        height: 30,
        x: 500,
        y: 100,
        toJSON: () => ({}),
      }) as DOMRect;
    trigger.dispatchEvent(new MouseEvent("mouseenter"));
    await Promise.resolve();

    const menu = document.body.querySelector(".text-select-menu") as HTMLUListElement;
    expect(menu).not.toBeNull();
    expect(menu.classList.contains("text-select-menu--left")).toBe(true);
    expect(menu.style.left).toBe("496px");
    expect(menu.style.transform).toBe("translateX(-100%)");

    trigger.dispatchEvent(new MouseEvent("mouseleave"));
    menu.dispatchEvent(new MouseEvent("mouseenter"));
    vi.advanceTimersByTime(200);
    expect(document.body.querySelector(".text-select-menu")).not.toBeNull();
  });

  test("keeps a portaled menu interactive inside a fixed-position parent", () => {
    const [value, setValue] = createSignal("a");
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(
      () => (
        <div style={{ position: "fixed" }}>
          <TextSelect
            portalMenu
            value={value()}
            options={[{ value: "a", label: "A" }, { value: "b", label: "B" }]}
            onChange={(event) => setValue(event.currentTarget.value)}
          />
        </div>
      ),
      container,
    );

    (container.querySelector(".text-select-trigger") as HTMLButtonElement).click();
    const option = [...document.body.querySelectorAll<HTMLButtonElement>(".text-select-option")]
      .find((element) => element.textContent === "B");
    expect(option).not.toBeUndefined();
    option?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    option?.click();

    expect(value()).toBe("b");
    expect(document.body.querySelector(".text-select-menu")).toBeNull();
  });
});
