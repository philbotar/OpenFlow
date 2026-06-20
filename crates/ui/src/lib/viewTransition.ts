import { batch } from "solid-js";
import { prefersReducedMotion } from "./motion";

export type NavTransitionType = "nav-forward" | "nav-back" | "nav-lateral";

export const SCREEN_VT_NAME = "app-screen";
export const PERSISTENT_SIDEBAR_VT = "persistent-sidebar";
export const PERSISTENT_HEADER_VT = "persistent-header";

export function supportsViewTransitions(): boolean {
  return (
    typeof document !== "undefined" &&
    typeof (document as Document & { startViewTransition?: unknown }).startViewTransition ===
      "function"
  );
}

export function withViewTransition(update: () => void): void {
  if (!supportsViewTransitions() || prefersReducedMotion()) {
    update();
    return;
  }

  (document as Document & {
    startViewTransition: (callback: () => void | Promise<void>) => {
      finished: Promise<void>;
    };
  }).startViewTransition(() => {
    batch(update);
  });
}
