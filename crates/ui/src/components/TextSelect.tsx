import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  splitProps,
  type JSX,
} from "solid-js";

export type TextSelectOption = {
  value: string;
  label: string;
};

type MenuPlacement = "auto" | "above" | "below";
type ResolvedMenuPlacement = Exclude<MenuPlacement, "auto">;

type TextSelectProps = {
  value: string;
  options: readonly TextSelectOption[];
  onChange?: (event: { currentTarget: { value: string } }) => void;
  disabled?: boolean;
  class?: string;
  classList?: JSX.HTMLAttributes<HTMLDivElement>["classList"];
  menuPlacement?: MenuPlacement;
  valuePrefix?: string;
  "aria-label"?: string;
};

type MenuStyle = {
  top: string;
  left: string;
  width: string;
  "max-height"?: string;
  transform?: string;
};

const MENU_GAP_PX = 4;
const MENU_MIN_WIDTH_PX = 180;

let nextListboxId = 0;

export function TextSelect(props: TextSelectProps) {
  const [local] = splitProps(props, ["class", "classList"]);
  const [open, setOpen] = createSignal(false);
  const [resolvedMenuPlacement, setResolvedMenuPlacement] =
    createSignal<ResolvedMenuPlacement>(
      props.menuPlacement === "above" ? "above" : "below",
    );
  const [menuStyle, setMenuStyle] = createSignal<MenuStyle>({
    top: "0px",
    left: "0px",
    width: "0px",
  });
  let rootRef: HTMLDivElement | undefined;
  let triggerRef: HTMLButtonElement | undefined;
  let menuRef: HTMLUListElement | undefined;
  const listboxId = `text-select-${++nextListboxId}`;

  const selectedLabel = createMemo(() => {
    const label =
      props.options.find((option) => option.value === props.value)?.label ?? props.value;
    return `${props.valuePrefix ?? ""}${label}`;
  });

  const syncMenuPosition = () => {
    const trigger = triggerRef;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const effectiveScale =
      trigger.offsetWidth > 0 ? rect.width / trigger.offsetWidth : 1;
    const positionScale =
      Number.isFinite(effectiveScale) && effectiveScale > 0 ? effectiveScale : 1;
    const left = rect.left / positionScale;
    const width = `${Math.max(trigger.offsetWidth, MENU_MIN_WIDTH_PX)}px`;
    const viewport = window.visualViewport;
    const viewportTop = viewport?.offsetTop ?? 0;
    const viewportHeight = viewport?.height ?? window.innerHeight;
    const viewportBottom = viewportTop + viewportHeight;
    const availableAbove = Math.max(0, rect.top - viewportTop);
    const availableBelow = Math.max(0, viewportBottom - rect.bottom);
    const menuHeight =
      menuRef?.isConnected ? menuRef.getBoundingClientRect().height : 0;
    const requestedPlacement = props.menuPlacement ?? "auto";
    const placement: ResolvedMenuPlacement =
      requestedPlacement === "above" ||
      (requestedPlacement === "auto" &&
        menuHeight > 0 &&
        availableBelow < menuHeight + MENU_GAP_PX * positionScale &&
        availableAbove > availableBelow)
        ? "above"
        : "below";
    const availableHeight = placement === "above" ? availableAbove : availableBelow;
    const constrainedMaxHeight =
      menuHeight > 0 &&
      availableHeight < menuHeight + MENU_GAP_PX * positionScale
        ? `${Math.max(0, availableHeight / positionScale - MENU_GAP_PX)}px`
        : undefined;
    setResolvedMenuPlacement(placement);
    if (placement === "above") {
      setMenuStyle({
        top: `${rect.top / positionScale - MENU_GAP_PX}px`,
        left: `${left}px`,
        width,
        "max-height": constrainedMaxHeight,
        transform: "translateY(-100%)",
      });
      return;
    }
    setMenuStyle({
      top: `${rect.bottom / positionScale + MENU_GAP_PX}px`,
      left: `${left}px`,
      width,
      "max-height": constrainedMaxHeight,
      transform: "none",
    });
  };

  const close = () => setOpen(false);

  const openMenu = () => {
    if (props.disabled) return;
    syncMenuPosition();
    setOpen(true);
  };

  const selectValue = (value: string) => {
    props.onChange?.({ currentTarget: { value } });
    close();
  };

  createEffect(() => {
    if (!open()) return;

    const onDocumentMouseDown = (event: MouseEvent) => {
      const root = rootRef;
      const target = event.target;
      if (!root || !(target instanceof Node) || root.contains(target)) return;
      close();
    };

    const onScroll = (event: Event) => {
      const root = rootRef;
      const target = event.target;
      if (root && target instanceof Node && root.contains(target)) return;
      close();
    };

    const onDismiss = () => close();

    document.addEventListener("mousedown", onDocumentMouseDown);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onDismiss);
    onCleanup(() => {
      document.removeEventListener("mousedown", onDocumentMouseDown);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onDismiss);
    });
  });

  return (
    <div
      ref={rootRef}
      class={`text-select-root${local.class ? ` ${local.class}` : ""}`}
      classList={local.classList}
    >
      <button
        ref={triggerRef}
        type="button"
        class="text-select-trigger"
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-controls={listboxId}
        aria-label={props["aria-label"]}
        disabled={props.disabled}
        onClick={() => (open() ? close() : openMenu())}
        onKeyDown={(event) => {
          if (props.disabled) return;
          if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            openMenu();
          }
          if (event.key === "Escape") {
            close();
          }
        }}
      >
        <span class="text-select-value">{selectedLabel()}</span>
      </button>
      <Show when={open()}>
        <ul
          ref={(element) => {
            menuRef = element;
            queueMicrotask(syncMenuPosition);
          }}
          id={listboxId}
          class="text-select-menu"
          classList={{ "text-select-menu--above": resolvedMenuPlacement() === "above" }}
          role="listbox"
          aria-label={props["aria-label"]}
          style={menuStyle()}
        >
          <For each={props.options}>
            {(option) => (
              <li role="presentation">
                <button
                  type="button"
                  class="text-select-option"
                  classList={{ "is-selected": option.value === props.value }}
                  role="option"
                  aria-selected={option.value === props.value}
                  title={option.label}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => selectValue(option.value)}
                >
                  <span class="text-select-option-label">{option.label}</span>
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
