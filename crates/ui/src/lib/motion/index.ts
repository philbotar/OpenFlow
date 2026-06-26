import { animate, type AnimationPlaybackControls } from "motion";

export function prefersReducedMotion(): boolean {
  return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

type MotionValue = string | number | readonly (string | number)[];
type MotionProps = Record<string, MotionValue>;

export function animateIn(
  element: Element,
  keyframes: MotionProps,
  options?: { duration?: number },
): AnimationPlaybackControls | undefined {
  if (prefersReducedMotion()) {
    return undefined;
  }
  return animate(element, keyframes, {
    duration: options?.duration ?? 0.25,
    ease: "easeOut",
  });
}

export function animateOut(
  element: Element,
  keyframes: MotionProps,
  options?: { duration?: number },
): Promise<void> {
  if (prefersReducedMotion()) {
    return Promise.resolve();
  }
  return animate(element, keyframes, {
    duration: options?.duration ?? 0.2,
    ease: "easeIn",
  }).finished;
}

export const modalEnterKeyframes = {
  opacity: [0, 1],
  scale: [0.96, 1],
};

export const modalExitKeyframes = {
  opacity: [1, 0],
  scale: [1, 0.96],
};

export const fadeEnterKeyframes = {
  opacity: [0, 1],
};

export const fadeExitKeyframes = {
  opacity: [1, 0],
};
