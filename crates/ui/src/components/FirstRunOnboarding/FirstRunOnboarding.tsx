import "./FirstRunOnboarding.css";
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";

import {
  IntroOverlay,
  SlideDescribe,
  SlideDone,
  SlideInspect,
  SlideRun,
  SlideWelcome,
} from "./onboardingSlides";

const INTRO_DURATION_MS = 5200;
const SLIDE_COUNT = 5;

export interface FirstRunOnboardingProps {
  open: boolean;
  onClose: () => void;
  onBuildWorkflow: () => Promise<void>;
  onSetupProvider: () => void;
}

export function FirstRunOnboarding(props: FirstRunOnboardingProps) {
  const [current, setCurrent] = createSignal(0);
  const [introPlaying, setIntroPlaying] = createSignal(false);
  const [launched, setLaunched] = createSignal(false);
  let introTimer: ReturnType<typeof setTimeout> | undefined;

  createEffect(() => {
    if (props.open) {
      setCurrent(0);
      setLaunched(false);
      setIntroPlaying(true);
      clearTimeout(introTimer);
      introTimer = setTimeout(() => setIntroPlaying(false), INTRO_DURATION_MS);
    } else {
      clearTimeout(introTimer);
      setIntroPlaying(false);
    }
  });

  onCleanup(() => clearTimeout(introTimer));

  const next = () => setCurrent((c) => Math.min(c + 1, SLIDE_COUNT - 1));
  const back = () => setCurrent((c) => Math.max(c - 1, 0));
  const skip = () => setCurrent(SLIDE_COUNT - 1);
  const dismissIntro = () => {
    clearTimeout(introTimer);
    setIntroPlaying(false);
  };

  const handleLaunch = async () => {
    setLaunched(true);
    await props.onBuildWorkflow();
    setLaunched(false);
  };

  return (
    <Show when={props.open}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Welcome to OpenFlow"
        class="of-onboarding"
        data-testid="first-run-onboarding"
      >
        <Show when={current() < SLIDE_COUNT - 1 && !introPlaying()}>
          <button type="button" class="of-onboarding-skip" onClick={skip}>
            Skip
          </button>
        </Show>

        <Show when={launched()}>
          <div class="of-launch-overlay">
            <div class="of-launch-row">
              <div class="of-spinner" />
              <span class="of-launch-text">Opening Openflow…</span>
            </div>
          </div>
        </Show>

        <Show when={introPlaying()}>
          <IntroOverlay onDismiss={dismissIntro} />
        </Show>

        <div
          class="of-carousel-track"
          style={{ "--of-slide-index": current() }}
        >
          <SlideWelcome />
          <SlideDescribe />
          <SlideInspect />
          <SlideRun />
          <SlideDone onBuildWorkflow={handleLaunch} onSetupProvider={props.onSetupProvider} />
        </div>

        <div class="of-footer">
          <div class="of-progress-dots">
            <For each={[0, 1, 2, 3, 4]}>
              {(i) => (
                <div class={`of-progress-dot ${i === current() ? "is-active" : ""}`} />
              )}
            </For>
          </div>
          <div class="of-footer-actions">
            <Show when={current() > 0}>
              <button type="button" class="of-btn-back" onClick={back}>
                ← Back
              </button>
            </Show>
            <Show when={current() < SLIDE_COUNT - 1}>
              <button type="button" class="of-btn-next" onClick={next}>
                Next →
              </button>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}
