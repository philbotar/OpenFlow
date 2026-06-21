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

type TextSelectProps = {
  value: string;
  options: readonly TextSelectOption[];
  onChange?: (event: { currentTarget: { value: string } }) => void;
  disabled?: boolean;
  class?: string;
  classList?: JSX.HTMLAttributes<HTMLDivElement>["classList"];
  "aria-label"?: string;
};

let nextListboxId = 0;

export function TextSelect(props: TextSelectProps) {
  const [local] = splitProps(props, ["class", "classList"]);
  const [open, setOpen] = createSignal(false);
  const [menuStyle, setMenuStyle] = createSignal<{ top: string; left: string; width: string }>({
    top: "0px",
    left: "0px",
    width: "0px",
  });
  let rootRef: HTMLDivElement | undefined;
  let triggerRef: HTMLButtonElement | undefined;
  const listboxId = `text-select-${++nextListboxId}`;

  const selectedLabel = createMemo(
    () => props.options.find((option) => option.value === props.value)?.label ?? props.value,
  );

  const syncMenuPosition = () => {
    const trigger = triggerRef;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    setMenuStyle({
      top: `${rect.bottom + 4}px`,
      left: `${rect.left}px`,
      width: `${rect.width}px`,
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
          id={listboxId}
          class="text-select-menu"
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
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => selectValue(option.value)}
                >
                  {option.label}
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
