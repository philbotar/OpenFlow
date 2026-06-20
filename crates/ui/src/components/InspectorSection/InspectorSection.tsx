import { createSignal, type JSX } from "solid-js";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { CollapsibleSection } from "../CollapsibleSection";
import { ICON_STROKE_WIDTH } from "../../lib/utils";

export function InspectorSection(props: {
  title: string;
  defaultOpen?: boolean;
  summary?: string;
  children: JSX.Element;
}) {
  const [open, setOpen] = createSignal(props.defaultOpen ?? false);

  return (
    <section class="inspector-section">
      <button
        type="button"
        class="inspector-section-header"
        aria-expanded={open()}
        onClick={() => setOpen((value) => !value)}
      >
        <ChevronRight
          class="inspector-section-chevron"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
        <span class="inspector-section-title">{props.title}</span>
        {props.summary ? (
          <span class="inspector-section-summary">{props.summary}</span>
        ) : null}
      </button>
      <CollapsibleSection open={open()}>
        <div class="inspector-section-body">{props.children}</div>
      </CollapsibleSection>
    </section>
  );
}
