const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => {
      if (el.tabIndex < 0 || el.matches(':disabled, input[type="hidden"]') ||
          el.closest('[hidden], [inert], [aria-hidden="true"]')) return false;
      for (let parent: HTMLElement | null = el; parent; parent = parent.parentElement) {
        const style = getComputedStyle(parent);
        if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") return false;
      }
      return true;
    },
  );
}

export function trapFocus(container: HTMLElement): () => void {
  const previousTabIndex = container.getAttribute("tabindex");
  if (previousTabIndex === null) container.tabIndex = -1;
  (getFocusableElements(container)[0] ?? container).focus();

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Tab" || event.defaultPrevented) {
      return;
    }
    const focusable = getFocusableElements(container);
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !focusable.includes(document.activeElement as HTMLElement)) {
      event.preventDefault();
      (event.shiftKey ? last ?? container : first ?? container).focus();
      return;
    }
    if (event.shiftKey) {
      if (document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      }
      return;
    }
    if (document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  container.addEventListener("keydown", handleKeyDown);
  return () => {
    container.removeEventListener("keydown", handleKeyDown);
    if (previousTabIndex === null) container.removeAttribute("tabindex");
  };
}
