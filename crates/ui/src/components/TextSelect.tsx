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
import { Portal } from "solid-js/web";

export type TextSelectOption = {
  value: string;
  label: string;
};

type MenuPlacement =
  | "auto"
  | "above"
  | "below"
  | "left"
  | "right"
  | "horizontal";
type ResolvedMenuPlacement = Exclude<MenuPlacement, "auto" | "horizontal">;

type TextSelectProps = {
  value: string;
  options: readonly TextSelectOption[];
  onChange?: (event: { currentTarget: { value: string } }) => void;
  disabled?: boolean;
  class?: string;
  classList?: JSX.HTMLAttributes<HTMLDivElement>["classList"];
  menuPlacement?: MenuPlacement;
  portalMenu?: boolean;
  openOnHover?: boolean;
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
  let hoverCloseTimer: number | undefined;
  let triggerHovered = false;
  let menuHovered = false;
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
    const viewport = window.visualViewport;
    const viewportLeft = viewport?.offsetLeft ?? 0;
    const viewportTop = viewport?.offsetTop ?? 0;
    const viewportWidth = viewport?.width ?? window.innerWidth;
    const viewportHeight = viewport?.height ?? window.innerHeight;
    const viewportRight = viewportLeft + viewportWidth;
    const viewportBottom = viewportTop + viewportHeight;
    const triggerLeft = rect.left / positionScale;
    const triggerRight = rect.right / positionScale;
    const triggerTop = rect.top / positionScale;
    const triggerBottom = rect.bottom / positionScale;
    const menuWidthPx = Math.max(trigger.offsetWidth, MENU_MIN_WIDTH_PX);
    const width = `${menuWidthPx}px`;
    const availableAbove = Math.max(0, rect.top - viewportTop);
    const availableBelow = Math.max(0, viewportBottom - rect.bottom);
    const rawMenuHeight =
      menuRef?.isConnected ? menuRef.getBoundingClientRect().height : 0;
    const menuHeight = rawMenuHeight / positionScale;
    const requestedPlacement = props.menuPlacement ?? "auto";
    const availableLeft = Math.max(
      0,
      triggerLeft - viewportLeft / positionScale,
    );
    const availableRight = Math.max(
      0,
      viewportRight / positionScale - triggerRight,
    );
    const sideFitsLeft = availableLeft >= menuWidthPx + MENU_GAP_PX;
    const sideFitsRight = availableRight >= menuWidthPx + MENU_GAP_PX;
    const placement: ResolvedMenuPlacement =
      requestedPlacement === "left" ||
      (requestedPlacement === "horizontal" &&
        !sideFitsRight &&
        (sideFitsLeft || availableLeft > availableRight))
        ? "left"
        : requestedPlacement === "right" ||
            (requestedPlacement === "horizontal" &&
              (sideFitsRight || !sideFitsLeft))
          ? "right"
          : requestedPlacement === "above" ||
              (requestedPlacement === "auto" &&
                rawMenuHeight > 0 &&
                availableBelow < rawMenuHeight + MENU_GAP_PX * positionScale &&
                availableAbove > availableBelow)
            ? "above"
            : "below";
    if (placement === "left" || placement === "right") {
      const verticalPadding = MENU_GAP_PX;
      const availableHeight = Math.max(
        0,
        viewportBottom / positionScale -
          viewportTop / positionScale -
          verticalPadding * 2,
      );
      const constrainedMaxHeight =
        menuHeight > availableHeight ? `${availableHeight}px` : undefined;
      const maxTop = Math.max(
        viewportTop / positionScale + verticalPadding,
        viewportBottom / positionScale - menuHeight - verticalPadding,
      );
      const top = Math.min(
        Math.max(triggerTop, viewportTop / positionScale + verticalPadding),
        maxTop,
      );
      setResolvedMenuPlacement(placement);
      setMenuStyle({
        top: `${top}px`,
        left: `${placement === "left" ? triggerLeft - MENU_GAP_PX : triggerRight + MENU_GAP_PX}px`,
        width,
        "max-height": constrainedMaxHeight,
        transform: placement === "left" ? "translateX(-100%)" : "none",
      });
      return;
    }
    const availableHeight =
      placement === "above" ? availableAbove : availableBelow;
    const constrainedMaxHeight =
      menuHeight > 0 &&
      availableHeight < rawMenuHeight + MENU_GAP_PX * positionScale
        ? `${Math.max(0, availableHeight / positionScale - MENU_GAP_PX)}px`
        : undefined;
    setResolvedMenuPlacement(placement);
    if (placement === "above") {
      setMenuStyle({
        top: `${triggerTop - MENU_GAP_PX}px`,
        left: `${triggerLeft}px`,
        width,
        "max-height": constrainedMaxHeight,
        transform: "translateY(-100%)",
      });
      return;
    }
    setMenuStyle({
      top: `${triggerBottom + MENU_GAP_PX}px`,
      left: `${triggerLeft}px`,
      width,
      "max-height": constrainedMaxHeight,
      transform: "none",
    });
  };

  const clearHoverClose = () => {
    if (hoverCloseTimer === undefined) return;
    window.clearTimeout(hoverCloseTimer);
    hoverCloseTimer = undefined;
  };

  const scheduleHoverClose = () => {
    if (!props.openOnHover || triggerHovered || menuHovered) return;
    clearHoverClose();
    hoverCloseTimer = window.setTimeout(() => {
      hoverCloseTimer = undefined;
      if (!triggerHovered && !menuHovered) setOpen(false);
    }, 140);
  };

  const close = () => {
    clearHoverClose();
    setOpen(false);
  };

  const openMenu = () => {
    if (props.disabled) return;
    clearHoverClose();
    syncMenuPosition();
    setOpen(true);
  };

  const selectValue = (value: string) => {
    props.onChange?.({ currentTarget: { value } });
    close();
  };

  onCleanup(clearHoverClose);

  createEffect(() => {
    if (!open()) return;

    const onDocumentMouseDown = (event: MouseEvent) => {
      const root = rootRef;
      const menu = menuRef;
      const target = event.target;
      if (
        !root ||
        !(target instanceof Node) ||
        root.contains(target) ||
        menu?.contains(target)
      ) {
        return;
      }
      close();
    };

    const onScroll = (event: Event) => {
      const root = rootRef;
      const menu = menuRef;
      const target = event.target;
      if (
        root &&
        target instanceof Node &&
        (root.contains(target) || menu?.contains(target))
      ) {
        return;
      }
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

  const renderMenu = () => (
    <ul
      ref={(element) => {
        menuRef = element;
        queueMicrotask(syncMenuPosition);
      }}
      id={listboxId}
      class="text-select-menu"
      classList={{
        "text-select-menu--above": resolvedMenuPlacement() === "above",
        "text-select-menu--left": resolvedMenuPlacement() === "left",
        "text-select-menu--right": resolvedMenuPlacement() === "right",
        "text-select-menu--portal": Boolean(props.portalMenu),
      }}
      role="listbox"
      aria-label={props["aria-label"]}
      style={menuStyle()}
      onMouseEnter={() => {
        if (!props.openOnHover) return;
        menuHovered = true;
        clearHoverClose();
      }}
      onMouseLeave={() => {
        if (!props.openOnHover) return;
        menuHovered = false;
        scheduleHoverClose();
      }}
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
              onMouseDown={(event) => {
                if (props.portalMenu) {
                  event.stopPropagation();
                  return;
                }
                event.preventDefault();
              }}
              onClick={() => selectValue(option.value)}
            >
              <span class="text-select-option-label">{option.label}</span>
            </button>
          </li>
        )}
      </For>
    </ul>
  );

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
        onMouseEnter={() => {
          if (!props.openOnHover) return;
          triggerHovered = true;
          clearHoverClose();
          openMenu();
        }}
        onMouseLeave={() => {
          if (!props.openOnHover) return;
          triggerHovered = false;
          scheduleHoverClose();
        }}
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
        {props.portalMenu ? <Portal>{renderMenu()}</Portal> : renderMenu()}
      </Show>
    </div>
  );
}
