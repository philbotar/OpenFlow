// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { trapFocus } from "./index";

let release: (() => void) | undefined;
afterEach(() => { release?.(); document.body.replaceChildren(); });

test("wraps across current enabled, visible controls after dialog content changes", () => {
  const dialog = document.createElement("section");
  dialog.innerHTML = '<button>First</button><button>Old last</button>';
  document.body.append(dialog);
  release = trapFocus(dialog);
  const first = dialog.querySelector("button")!;
  expect(document.activeElement).toBe(first);
  dialog.innerHTML = '<button hidden>Hidden</button><div hidden><button>Hidden child</button></div><button disabled>Disabled</button><button tabindex="-1">Excluded</button><input type="hidden"><button>New first</button><button>New last</button>';
  const buttons = dialog.querySelectorAll("button");
  const last = buttons[buttons.length - 1];
  const nextFirst = buttons[buttons.length - 2];
  last.focus();
  last.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
  expect(document.activeElement).toBe(nextFirst);
  nextFirst.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true, cancelable: true }));
  expect(document.activeElement).toBe(last);
});

test("keeps an empty dialog focusable, then discovers controls added later", () => {
  const dialog = document.createElement("section");
  document.body.append(dialog);
  release = trapFocus(dialog);
  expect(document.activeElement).toBe(dialog);
  const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
  dialog.dispatchEvent(tab);
  expect(tab.defaultPrevented).toBe(true);
  dialog.innerHTML = '<button>Ready</button>';
  dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
  expect(document.activeElement).toBe(dialog.querySelector("button"));
  release();
  expect(dialog.hasAttribute("tabindex")).toBe(false);
});
