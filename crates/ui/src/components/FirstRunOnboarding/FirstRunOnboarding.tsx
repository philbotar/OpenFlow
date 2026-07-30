import "./FirstRunOnboarding.css";
import { createEffect, createSignal, onCleanup, Show } from "solid-js";

type TourPlacement = "top" | "right" | "bottom" | "left" | "inside-top-left";

type TourStep = {
  id: string;
  selectors: string[];
  title: string;
  description: string;
  placement: TourPlacement;
  prepare?: "workflow" | "inspector" | "workflow-settings";
};

const TOUR_STEPS: TourStep[] = [
  {
    id: "new-workflow",
    selectors: ['[data-tour="new-workflow"]'],
    title: "Create a workflow",
    description:
      "Choose a blank workflow or Create with AI. Workflows are reusable, multi-step jobs.",
    placement: "right",
  },
  {
    id: "new-chat",
    selectors: ['[data-tour="new-chat"]'],
    title: "Use chats for one-off work",
    description:
      "Chats are free-form conversations. They keep history but have no workflow graph.",
    placement: "right",
  },
  {
    id: "workflow-library",
    selectors: ['[data-tour="workflow-library"]'],
    title: "Find saved workflows",
    description: "Select a saved workflow here to edit or run it.",
    placement: "right",
  },
  {
    id: "workflow-canvas",
    selectors: ['[data-tour="workflow-canvas"]'],
    title: "Inspect the real workflow",
    description:
      "Each node is an AI step. Edges control how work passes between steps.",
    placement: "inside-top-left",
    prepare: "workflow",
  },
  {
    id: "node-inspector",
    selectors: [
      '[data-tour="node-inspector-button"]',
      '[data-tour="node-inspector-panel"]',
    ],
    title: "Configure a step",
    description:
      "Edit the selected node's prompt, model, tools, skills, and handoff.",
    placement: "left",
    prepare: "inspector",
  },
  {
    id: "workflow-settings",
    selectors: [
      '[data-tour="workflow-settings-button"]',
      '[data-tour="workflow-settings-panel"]',
    ],
    title: "Set workflow-wide defaults",
    description:
      "Choose shared provider, context, planning mode, and schedule here.",
    placement: "left",
    prepare: "workflow-settings",
  },
  {
    id: "run-workflow",
    selectors: ['[data-tour="run-workflow"]'],
    title: "Run the workflow",
    description:
      "Run starts dependency-ready nodes and shows progress on the graph.",
    placement: "bottom",
    prepare: "workflow",
  },
  {
    id: "workflow-composer",
    selectors: ['[data-tour="workflow-composer"]'],
    title: "Guide the run here",
    description:
      "Add kickoff context, attach files, answer pauses, or continue a run.",
    placement: "top",
    prepare: "workflow",
  },
  {
    id: "settings",
    selectors: ['[data-tour="settings"]'],
    title: "Connect an AI provider",
    description:
      "Add provider keys and configure models, MCP, and appearance.",
    placement: "right",
  },
  {
    id: "help",
    selectors: ['[data-tour="help"]'],
    title: "Replay this tour anytime",
    description: "Open Help whenever a teammate needs this walkthrough.",
    placement: "right",
  },
];

const TARGET_PADDING = 6;
const TOOLTIP_GAP = 14;
const VIEWPORT_PADDING = 16;

type Point = {
  left: number;
  top: number;
};

type HighlightRect = Point & {
  width: number;
  height: number;
};

export interface FirstRunOnboardingProps {
  open: boolean;
  onClose: () => void;
  onShowWorkflow: () => void;
  onShowInspector: () => void;
  onShowWorkflowSettings: () => void;
}

function findTarget(step: TourStep) {
  for (const selector of step.selectors) {
    const target = document.querySelector(selector);
    if (target instanceof HTMLElement) {
      return target;
    }
  }
  return null;
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

function tooltipPosition(
  target: DOMRect,
  tooltip: DOMRect,
  placement: TourPlacement,
): Point {
  const maxLeft = Math.max(VIEWPORT_PADDING, window.innerWidth - tooltip.width - VIEWPORT_PADDING);
  const maxTop = Math.max(VIEWPORT_PADDING, window.innerHeight - tooltip.height - VIEWPORT_PADDING);
  let left = target.left + (target.width - tooltip.width) / 2;
  let top = target.bottom + TOOLTIP_GAP;

  if (placement === "top") {
    top = target.top - tooltip.height - TOOLTIP_GAP;
  } else if (placement === "right") {
    left = target.right + TOOLTIP_GAP;
    top = target.top + (target.height - tooltip.height) / 2;
  } else if (placement === "left") {
    left = target.left - tooltip.width - TOOLTIP_GAP;
    top = target.top + (target.height - tooltip.height) / 2;
  } else if (placement === "inside-top-left") {
    left = target.left + 20;
    top = target.top + 20;
  }

  if (placement === "top" && top < VIEWPORT_PADDING) {
    top = target.bottom + TOOLTIP_GAP;
  } else if (placement === "bottom" && top + tooltip.height > window.innerHeight - VIEWPORT_PADDING) {
    top = target.top - tooltip.height - TOOLTIP_GAP;
  } else if (placement === "right" && left + tooltip.width > window.innerWidth - VIEWPORT_PADDING) {
    left = target.left - tooltip.width - TOOLTIP_GAP;
  } else if (placement === "left" && left < VIEWPORT_PADDING) {
    left = target.right + TOOLTIP_GAP;
  }

  return {
    left: clamp(left, VIEWPORT_PADDING, maxLeft),
    top: clamp(top, VIEWPORT_PADDING, maxTop),
  };
}

export function FirstRunOnboarding(props: FirstRunOnboardingProps) {
  const [stepIndex, setStepIndex] = createSignal(0);
  const [targetId, setTargetId] = createSignal<string | null>(null);
  const [targetRect, setTargetRect] = createSignal<HighlightRect | null>(null);
  const [tooltipPoint, setTooltipPoint] = createSignal<Point>({
    left: VIEWPORT_PADDING,
    top: VIEWPORT_PADDING,
  });
  let target: HTMLElement | null = null;
  let tooltipRef: HTMLElement | undefined;
  let syncFrame: number | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let moveDirection: 1 | -1 = 1;
  let wasOpen = false;

  const currentStep = () => TOUR_STEPS[stepIndex()];

  const clearTarget = () => {
    target = null;
    resizeObserver?.disconnect();
    resizeObserver = undefined;
    setTargetId(null);
    setTargetRect(null);
  };

  const updateGeometry = () => {
    if (!target || !tooltipRef) return;
    const rect = target.getBoundingClientRect();
    const tooltipRect = tooltipRef.getBoundingClientRect();
    setTargetRect({
      left: Math.max(0, rect.left - TARGET_PADDING),
      top: Math.max(0, rect.top - TARGET_PADDING),
      width: Math.min(
        window.innerWidth - Math.max(0, rect.left - TARGET_PADDING),
        rect.width + TARGET_PADDING * 2,
      ),
      height: Math.min(
        window.innerHeight - Math.max(0, rect.top - TARGET_PADDING),
        rect.height + TARGET_PADDING * 2,
      ),
    });
    setTooltipPoint(tooltipPosition(rect, tooltipRect, currentStep().placement));
  };

  const scheduleGeometryUpdate = () => {
    if (syncFrame !== undefined) cancelAnimationFrame(syncFrame);
    syncFrame = requestAnimationFrame(() => {
      syncFrame = undefined;
      updateGeometry();
    });
  };

  const syncCurrentStep = () => {
    if (!props.open) return;

    let candidate = stepIndex();
    let nextTarget = findTarget(TOUR_STEPS[candidate]);
    while (!nextTarget) {
      candidate += moveDirection;
      if (candidate < 0 || candidate >= TOUR_STEPS.length) {
        props.onClose();
        return;
      }
      nextTarget = findTarget(TOUR_STEPS[candidate]);
    }

    if (candidate !== stepIndex()) {
      setStepIndex(candidate);
      return;
    }

    target = nextTarget;
    setTargetId(target.dataset.tour ?? null);
    target.scrollIntoView?.({ block: "nearest", inline: "nearest" });
    resizeObserver?.disconnect();
    if (globalThis.ResizeObserver) {
      resizeObserver = new ResizeObserver(scheduleGeometryUpdate);
      resizeObserver.observe(target);
    }
    scheduleGeometryUpdate();
  };

  const scheduleStepSync = () => {
    if (syncFrame !== undefined) cancelAnimationFrame(syncFrame);
    syncFrame = requestAnimationFrame(() => {
      syncFrame = undefined;
      syncCurrentStep();
    });
  };

  const move = (direction: 1 | -1) => {
    const nextIndex = stepIndex() + direction;
    if (nextIndex < 0) return;
    if (nextIndex >= TOUR_STEPS.length) {
      props.onClose();
      return;
    }
    const nextStep = TOUR_STEPS[nextIndex];
    if (nextStep.prepare === "workflow") {
      props.onShowWorkflow();
    } else if (nextStep.prepare === "inspector") {
      props.onShowInspector();
    } else if (nextStep.prepare === "workflow-settings") {
      props.onShowWorkflowSettings();
    }
    moveDirection = direction;
    clearTarget();
    setStepIndex(nextIndex);
  };

  createEffect(() => {
    const open = props.open;
    if (open && !wasOpen) {
      moveDirection = 1;
      setStepIndex(0);
      scheduleStepSync();
    } else if (!open) {
      clearTarget();
    }
    wasOpen = open;
  });

  createEffect(() => {
    stepIndex();
    if (props.open) scheduleStepSync();
  });

  const handleViewportChange = () => scheduleGeometryUpdate();
  const handleKeyDown = (event: KeyboardEvent) => {
    if (!props.open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      props.onClose();
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      move(1);
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      move(-1);
    }
  };

  createEffect(() => {
    if (!props.open) return;
    window.addEventListener("resize", handleViewportChange);
    window.addEventListener("scroll", handleViewportChange, true);
    window.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      window.removeEventListener("resize", handleViewportChange);
      window.removeEventListener("scroll", handleViewportChange, true);
      window.removeEventListener("keydown", handleKeyDown);
      if (syncFrame !== undefined) cancelAnimationFrame(syncFrame);
      resizeObserver?.disconnect();
    });
  });

  return (
    <Show when={props.open}>
      <div
        class="of-tour-layer"
        data-testid="first-run-onboarding"
        data-tour-step={currentStep().id}
        data-tour-target={targetId() ?? undefined}
      >
        <Show when={targetRect()}>
          {(rect) => (
            <div
              class="of-tour-highlight"
              style={{
                left: `${rect().left}px`,
                top: `${rect().top}px`,
                width: `${rect().width}px`,
                height: `${rect().height}px`,
              }}
              aria-hidden="true"
            />
          )}
        </Show>
        <section
          ref={tooltipRef}
          class="of-tour-tooltip"
          style={{
            left: `${tooltipPoint().left}px`,
            top: `${tooltipPoint().top}px`,
          }}
          role="dialog"
          aria-modal="false"
          aria-labelledby="of-tour-title"
          aria-describedby="of-tour-description"
        >
          <div class="of-tour-meta">
            <span>Quick tour</span>
            <span>
              {stepIndex() + 1} / {TOUR_STEPS.length}
            </span>
          </div>
          <h2 id="of-tour-title">{currentStep().title}</h2>
          <p id="of-tour-description">{currentStep().description}</p>
          <div class="of-tour-actions">
            <button type="button" class="of-tour-skip" onClick={props.onClose}>
              Skip tour
            </button>
            <div class="of-tour-step-actions">
              <Show when={stepIndex() > 0}>
                <button type="button" class="of-tour-back" onClick={() => move(-1)}>
                  Back
                </button>
              </Show>
              <button type="button" class="of-tour-next" onClick={() => move(1)}>
                {stepIndex() === TOUR_STEPS.length - 1 ? "Done" : "Next"}
              </button>
            </div>
          </div>
        </section>
      </div>
    </Show>
  );
}
