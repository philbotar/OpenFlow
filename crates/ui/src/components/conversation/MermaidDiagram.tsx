import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import { Show, createEffect, createSignal, createUniqueId, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import type { RenderResult } from "mermaid";
import { AnimatedModal } from "../AnimatedModal";

interface MermaidDiagramProps {
  source: string;
}

type DiagramState =
  | { status: "loading" }
  | { status: "rendered"; result: RenderResult }
  | { status: "error" };

type ColorScheme = "light" | "dark";
type RgbColor = readonly [red: number, green: number, blue: number];

const MIN_TEXT_CONTRAST = 4.5;

const FALLBACK_COLORS = {
  light: {
    background: "#f6f3ed",
    primary: "#f1ede5",
    secondary: "#ece7dd",
    tertiary: "#f7f4ee",
    text: "#18181b",
    secondaryText: "#66645d",
    tertiaryText: "#8a877f",
    line: "#c3cbda",
  },
  dark: {
    background: "#101010",
    primary: "#242425",
    secondary: "#303032",
    tertiary: "#1b1b1c",
    text: "#f2f2f3",
    secondaryText: "#b0b0b3",
    tertiaryText: "#85858b",
    line: "#55555c",
  },
} as const;

function currentColorScheme(): ColorScheme {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

function parseCssColor(value: string): RgbColor | undefined {
  const hex = value.match(/^#([\da-f]{6})$/i)?.[1];
  if (hex) {
    return [
      Number.parseInt(hex.slice(0, 2), 16),
      Number.parseInt(hex.slice(2, 4), 16),
      Number.parseInt(hex.slice(4, 6), 16),
    ];
  }

  const rgb = value.match(/^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/i);
  if (!rgb) return undefined;
  return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
}

function relativeLuminance([red, green, blue]: RgbColor): number {
  const linear = [red, green, blue].map((channel) => {
    const normalized = channel / 255;
    return normalized <= 0.04045
      ? normalized / 12.92
      : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrastRatio(first: RgbColor, second: RgbColor): number {
  const lighter = Math.max(relativeLuminance(first), relativeLuminance(second));
  const darker = Math.min(relativeLuminance(first), relativeLuminance(second));
  return (lighter + 0.05) / (darker + 0.05);
}

function ensureReadableDiagramLabels(container: HTMLElement): void {
  const darkText = parseCssColor(FALLBACK_COLORS.light.text)!;
  const lightText = parseCssColor(FALLBACK_COLORS.dark.text)!;

  for (const item of container.querySelectorAll<SVGGElement>(".node, .cluster")) {
    const shape = item.querySelector<SVGGraphicsElement>(
      ":scope > rect, :scope > polygon, :scope > path, :scope > circle, :scope > ellipse, .label-container",
    );
    const label = item.querySelector<HTMLElement | SVGElement>(
      ".nodeLabel, .label text, .cluster-label text",
    );
    if (!shape || !label) continue;

    const background = parseCssColor(getComputedStyle(shape).fill);
    const labelStyles = getComputedStyle(label);
    const currentText = parseCssColor(labelStyles.color) ?? parseCssColor(labelStyles.fill);
    if (!background || !currentText) continue;
    if (contrastRatio(background, currentText) >= MIN_TEXT_CONTRAST) continue;

    const darkContrast = contrastRatio(background, darkText);
    const lightContrast = contrastRatio(background, lightText);
    const replacement =
      darkContrast >= lightContrast ? FALLBACK_COLORS.light.text : FALLBACK_COLORS.dark.text;

    for (const element of item.querySelectorAll<HTMLElement | SVGElement>(
      ".nodeLabel, .nodeLabel *, .label text, .label tspan, .cluster-label text, .cluster-label tspan",
    )) {
      element.style.setProperty("color", replacement, "important");
      element.style.setProperty("fill", replacement, "important");
    }
  }
}

function themeVariables(scheme: ColorScheme) {
  const styles = getComputedStyle(document.documentElement);
  const fallback = FALLBACK_COLORS[scheme];
  const read = (name: string, defaultValue: string) =>
    styles.getPropertyValue(name).trim() || defaultValue;
  const background = read("--base-sand-100", fallback.background);
  const primary = read("--base-sand-500", fallback.primary);
  const secondary = read("--base-sand-300", fallback.secondary);
  const tertiary = read("--base-sand-400", fallback.tertiary);
  const text = read("--base-ink-900", fallback.text);
  const secondaryText = read("--base-ink-700", fallback.secondaryText);
  const tertiaryText = read("--base-ink-500", fallback.tertiaryText);
  const line = read("--canvas-edge-stroke", fallback.line);

  return {
    background,
    primaryColor: primary,
    secondaryColor: secondary,
    tertiaryColor: tertiary,
    primaryTextColor: text,
    secondaryTextColor: secondaryText,
    tertiaryTextColor: tertiaryText,
    primaryBorderColor: line,
    secondaryBorderColor: line,
    tertiaryBorderColor: line,
    lineColor: line,
    arrowheadColor: line,
    textColor: text,
    nodeBkg: primary,
    mainBkg: primary,
    nodeBorder: line,
    clusterBkg: background,
    clusterBorder: line,
    defaultLinkColor: line,
    titleColor: text,
    edgeLabelBackground: tertiary,
    nodeTextColor: text,
    noteBkgColor: secondary,
    noteTextColor: text,
    noteBorderColor: line,
    actorBkg: primary,
    actorBorder: line,
    actorTextColor: text,
    actorLineColor: line,
    signalColor: line,
    signalTextColor: text,
    labelBoxBkgColor: tertiary,
    labelBoxBorderColor: line,
    labelTextColor: text,
    labelBackground: tertiary,
    fontFamily: styles.fontFamily,
  };
}

export function MermaidDiagram(props: MermaidDiagramProps) {
  const baseId = `mermaid-${createUniqueId()}`;
  const [colorScheme, setColorScheme] = createSignal(currentColorScheme());
  const [fullScreen, setFullScreen] = createSignal(false);
  const [state, setState] = createSignal<DiagramState>({ status: "loading" });
  let renderSequence = 0;

  onMount(() => {
    const observer = new MutationObserver(() => setColorScheme(currentColorScheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    const source = props.source;
    const activeColorScheme = colorScheme();
    const renderId = `${baseId}-${renderSequence++}`;
    let active = true;

    setState({ status: "loading" });
    void import("mermaid")
      .then(async ({ default: mermaid }) => {
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          suppressErrorRendering: true,
          theme: "base",
          themeVariables: themeVariables(activeColorScheme),
        });
        return mermaid.render(renderId, source);
      })
      .then((result) => {
        if (active) setState({ status: "rendered", result });
      })
      .catch(() => {
        if (active) setState({ status: "error" });
      });

    onCleanup(() => {
      active = false;
    });
  });

  const rendered = () => {
    const current = state();
    return current.status === "rendered" ? current.result : undefined;
  };

  return (
    <Show
      when={rendered()}
      fallback={
        <Show
          when={state().status === "error"}
          fallback={
            <div class="mermaid-diagram mermaid-diagram--loading" role="status">
              Rendering diagram…
            </div>
          }
        >
          <div class="mermaid-diagram-error">
            <div>Could not render Mermaid diagram.</div>
            <pre>
              <code class="language-mermaid">{props.source}</code>
            </pre>
          </div>
        </Show>
      }
    >
      {(result) => (
        <>
          <div class="mermaid-diagram-shell">
            <div
              ref={(element) => {
                result().bindFunctions?.(element);
                queueMicrotask(() => ensureReadableDiagramLabels(element));
              }}
              class="mermaid-diagram"
              role="img"
              aria-label="Mermaid diagram"
              innerHTML={result().svg}
            />
            <button
              type="button"
              class="mermaid-diagram-fullscreen-trigger"
              aria-label="View Mermaid diagram full screen"
              aria-haspopup="dialog"
              title="View full screen"
              onClick={() => setFullScreen(true)}
            >
              <Maximize2 aria-hidden="true" width={16} height={16} />
            </button>
          </div>
          <Portal>
            <AnimatedModal
              open={fullScreen()}
              onClose={() => setFullScreen(false)}
              ariaLabel="Mermaid diagram full screen"
              backdropClass="mermaid-fullscreen-backdrop"
              cardClass="mermaid-fullscreen-card"
            >
              <div
                ref={(element) => {
                  result().bindFunctions?.(element);
                  queueMicrotask(() => ensureReadableDiagramLabels(element));
                }}
                class="mermaid-fullscreen-diagram"
                role="img"
                aria-label="Mermaid diagram, full screen"
                innerHTML={result().svg}
              />
              <button
                type="button"
                class="mermaid-fullscreen-close"
                aria-label="Exit full screen"
                title="Exit full screen"
                onClick={() => setFullScreen(false)}
              >
                <Minimize2 aria-hidden="true" width={18} height={18} />
              </button>
            </AnimatedModal>
          </Portal>
        </>
      )}
    </Show>
  );
}
