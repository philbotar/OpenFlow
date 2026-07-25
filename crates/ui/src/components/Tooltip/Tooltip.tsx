import { Show, createSignal, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { formatShortcutParts, type ShortcutId } from "../../lib/shortcuts";
import "./Tooltip.css";

const SHOW_DELAY_MS = 400;

export type TooltipProps = {
  label: string;
  shortcutId?: ShortcutId;
  disabledReason?: string;
  children: JSX.Element;
};

export function Tooltip(props: TooltipProps) {
  const [open, setOpen] = createSignal(false);
  const [coords, setCoords] = createSignal({ top: 0, left: 0 });
  let timer: number | undefined;

  const clearTimer = () => {
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timer = undefined;
    }
  };

  const hide = () => {
    clearTimer();
    setOpen(false);
  };

  const scheduleShow = (el: HTMLElement) => {
    clearTimer();
    timer = window.setTimeout(() => {
      const rect = el.getBoundingClientRect();
      const tipHeight = 32;
      const spaceBelow = window.innerHeight - rect.bottom;
      const top =
        spaceBelow < tipHeight + 8 ? rect.top - tipHeight - 6 : rect.bottom + 6;
      setCoords({ top, left: rect.left + rect.width / 2 });
      setOpen(true);
    }, SHOW_DELAY_MS);
  };

  onCleanup(hide);

  const text = () => props.disabledReason ?? props.label;
  const parts = () =>
    props.disabledReason || !props.shortcutId
      ? []
      : formatShortcutParts(props.shortcutId);

  const onPointerEnter = (e: PointerEvent) => {
    const el = e.currentTarget;
    if (el instanceof HTMLElement) scheduleShow(el);
  };

  const onFocusIn = (e: FocusEvent) => {
    const el = e.currentTarget;
    if (el instanceof HTMLElement) scheduleShow(el);
  };

  return (
    <>
      <span
        class="app-tooltip-trigger"
        onPointerEnter={onPointerEnter}
        onPointerLeave={hide}
        onFocusIn={onFocusIn}
        onFocusOut={hide}
        onPointerDown={hide}
      >
        {props.children}
      </span>
      <Show when={open()}>
        <Portal>
          <div
            class="app-tooltip"
            style={{
              top: `${coords().top}px`,
              left: `${coords().left}px`,
            }}
            role="tooltip"
          >
            <span class="app-tooltip-label">{text()}</span>
            <Show when={parts().length > 0}>
              <span class="app-tooltip-keys" aria-hidden="true">
                {parts().map((part) => (
                  <kbd class="app-tooltip-key">{part}</kbd>
                ))}
              </span>
            </Show>
          </div>
        </Portal>
      </Show>
    </>
  );
}
