/** @jsxImportSource react */
/** @jsxRuntime automatic */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactElement,
} from "react";
import { createPortal } from "react-dom";
import { formatShortcutParts, type ShortcutId } from "../lib/shortcuts";
import "../components/Tooltip/Tooltip.css";

const SHOW_DELAY_MS = 400;

export type AppTooltipProps = {
  label: string;
  shortcutId?: ShortcutId;
  side?: "auto" | "right";
  children: ReactElement;
};

export function AppTooltip({
  label,
  shortcutId,
  side = "auto",
  children,
}: AppTooltipProps): ReactElement {
  const triggerRef = useRef<HTMLSpanElement>(null);
  const timerRef = useRef<number | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState({ top: 0, left: 0 });

  const clearTimer = useCallback(() => {
    if (timerRef.current !== undefined) {
      window.clearTimeout(timerRef.current);
      timerRef.current = undefined;
    }
  }, []);

  const hide = useCallback(() => {
    clearTimer();
    setOpen(false);
  }, [clearTimer]);

  const triggerRect = useCallback((el: HTMLElement): DOMRect => {
    const child = el.firstElementChild;
    if (child instanceof HTMLElement) {
      return child.getBoundingClientRect();
    }
    return el.getBoundingClientRect();
  }, []);

  const scheduleShow = useCallback(() => {
    const el = triggerRef.current;
    if (!el) return;
    clearTimer();
    timerRef.current = window.setTimeout(() => {
      const rect = triggerRect(el);
      if (side === "right") {
        setCoords({
          top: rect.top + rect.height / 2,
          left: rect.right + 6,
        });
        setOpen(true);
        return;
      }
      const tipHeight = 32;
      const spaceBelow = window.innerHeight - rect.bottom;
      const top =
        spaceBelow < tipHeight + 8 ? rect.top - tipHeight - 6 : rect.bottom + 6;
      setCoords({ top, left: rect.left + rect.width / 2 });
      setOpen(true);
    }, SHOW_DELAY_MS);
  }, [clearTimer, side, triggerRect]);

  useEffect(() => () => hide(), [hide]);

  const parts = shortcutId ? formatShortcutParts(shortcutId) : [];

  return (
    <>
      <span
        ref={triggerRef}
        className="app-tooltip-trigger"
        onPointerEnter={scheduleShow}
        onPointerLeave={hide}
        onFocus={scheduleShow}
        onBlur={hide}
        onPointerDown={hide}
      >
        {children}
      </span>
      {open &&
        createPortal(
          <div
            className="app-tooltip"
            style={{
              top: `${coords.top}px`,
              left: `${coords.left}px`,
            }}
            role="tooltip"
            data-side={side}
          >
            <span className="app-tooltip-label">{label}</span>
            {parts.length > 0 ? (
              <span className="app-tooltip-keys" aria-hidden="true">
                {parts.map((part) => (
                  <kbd key={part} className="app-tooltip-key">
                    {part}
                  </kbd>
                ))}
              </span>
            ) : null}
          </div>,
          document.body,
        )}
    </>
  );
}
