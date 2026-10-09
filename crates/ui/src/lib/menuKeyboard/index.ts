/** Keyboard behavior shared by the sidebar's native menu buttons. */
export function handleMenuKeyDown(event: KeyboardEvent): void {
  const root = event.currentTarget;
  if (!(root instanceof HTMLElement)) return;
  const trigger = root.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');
  if (!trigger) return;
  const onTrigger = event.target === trigger;
  if (onTrigger && ["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
    event.preventDefault();
    if (trigger.getAttribute("aria-expanded") !== "true") trigger.click();
    const items = root.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)');
    items[event.key === "ArrowUp" ? items.length - 1 : 0]?.focus();
    return;
  }
  const menu = root.querySelector('[role="menu"]');
  if (!menu?.contains(event.target as Node)) return;
  if (event.key === "Escape" || event.key === "Tab") {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
    }
    trigger.focus();
    trigger.click();
    return;
  }
  const items = Array.from(menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'));
  const current = items.indexOf(document.activeElement as HTMLButtonElement);
  const next = event.key === "Home" ? 0
    : event.key === "End" ? items.length - 1
    : event.key === "ArrowDown" ? (current + 1) % items.length
    : event.key === "ArrowUp" ? (current - 1 + items.length) % items.length : undefined;
  if (next !== undefined) {
    event.preventDefault();
    items[next]?.focus();
  }
}
