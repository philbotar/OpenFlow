import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import type { JSX } from "solid-js";
import { trapFocus } from "../lib/focusTrap";
import {
  animateIn,
  animateOut,
  fadeEnterKeyframes,
  fadeExitKeyframes,
  modalEnterKeyframes,
  modalExitKeyframes,
} from "../lib/motion";

interface AnimatedModalProps {
  open: boolean;
  onClose: () => void;
  ariaLabel: string;
  backdropClass?: string;
  cardClass?: string;
  children: JSX.Element;
}

export function AnimatedModal(props: AnimatedModalProps) {
  const [rendered, setRendered] = createSignal(false);
  let backdropRef: HTMLDivElement | undefined;
  let cardRef: HTMLElement | undefined;
  let releaseFocusTrap: (() => void) | undefined;
  let previousFocus: HTMLElement | null = null;
  let entered = false;

  const cleanupFocusTrap = () => {
    releaseFocusTrap?.();
    releaseFocusTrap = undefined;
  };

  const runEnter = () => {
    if (!backdropRef || !cardRef || entered) {
      return;
    }
    entered = true;
    animateIn(backdropRef, fadeEnterKeyframes, { duration: 0.2 });
    animateIn(cardRef, modalEnterKeyframes);
    previousFocus = document.activeElement as HTMLElement | null;
    releaseFocusTrap = trapFocus(cardRef);
  };

  const runExit = () => {
    if (!backdropRef || !cardRef) {
      setRendered(false);
      entered = false;
      cleanupFocusTrap();
      previousFocus?.focus();
      return;
    }
    void Promise.all([
      animateOut(backdropRef, fadeExitKeyframes, { duration: 0.15 }),
      animateOut(cardRef, modalExitKeyframes, { duration: 0.15 }),
    ]).then(() => {
      setRendered(false);
      entered = false;
      cleanupFocusTrap();
      previousFocus?.focus();
    });
  };

  createEffect(() => {
    if (props.open) {
      setRendered(true);
      queueMicrotask(runEnter);
      return;
    }
    if (rendered()) {
      runExit();
    }
  });

  onMount(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && props.open) {
        event.preventDefault();
        props.onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => {
      window.removeEventListener("keydown", onKeyDown);
      cleanupFocusTrap();
    });
  });

  return (
    <Show when={rendered()}>
      <div
        ref={backdropRef}
        class={props.backdropClass ?? "node-picker-backdrop"}
        onClick={() => props.onClose()}
      >
        <section
          ref={cardRef}
          class={props.cardClass ?? "node-picker-card"}
          role="dialog"
          aria-modal="true"
          aria-label={props.ariaLabel}
          onClick={(event) => event.stopPropagation()}
        >
          {props.children}
        </section>
      </div>
    </Show>
  );
}
