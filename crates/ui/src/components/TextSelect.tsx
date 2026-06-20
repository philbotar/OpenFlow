import type { JSX } from "solid-js";

export function TextSelect(props: JSX.SelectHTMLAttributes<HTMLSelectElement>) {
  return <select class="text-select" {...props} />;
}
