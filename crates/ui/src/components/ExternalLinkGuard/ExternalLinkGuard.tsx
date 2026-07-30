import { onCleanup, onMount } from "solid-js";
import type { ParentProps } from "solid-js";
import { openExternalUrl } from "../../api";

const EXTERNAL_PROTOCOLS = new Set(["http:", "https:", "mailto:", "tel:"]);

function handleDocumentClick(event: MouseEvent) {
  if (event.defaultPrevented) return;

  const target = event.target;
  const anchor = target instanceof Element ? target.closest("a[href]") : null;
  const href = anchor?.getAttribute("href")?.trim();
  if (!href || href.startsWith("#")) return;

  // Never let a link replace the OpenFlow WebView, including unsupported
  // schemes. Only known external schemes are handed to the OS.
  event.preventDefault();

  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return;
  }
  if (!EXTERNAL_PROTOCOLS.has(url.protocol)) return;

  void openExternalUrl(url.href).catch((error: unknown) => {
    console.error("Failed to open external link", error);
  });
}

export function ExternalLinkGuard(props: ParentProps) {
  onMount(() => {
    document.addEventListener("click", handleDocumentClick);
    onCleanup(() => document.removeEventListener("click", handleDocumentClick));
  });

  return <>{props.children}</>;
}
