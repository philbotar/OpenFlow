// @vitest-environment jsdom
import { render } from "solid-js/web";
import { describe, expect, it } from "vitest";
import { Message } from "./Message";

function renderMessage(props: Parameters<typeof Message>[0]) {
  const container = document.createElement("div");
  document.body.append(container);
  const dispose = render(() => <Message {...props} />, container);
  return { container, dispose };
}

describe("Message", () => {
  it("does not animate streaming assistant rows", () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Working through it",
      streaming: true,
    });

    const row = container.querySelector(".message-assistant");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    expect(container.querySelector(".message-streaming-caret")).not.toBeNull();
    dispose();
  });

  it("does not animate completed assistant rows", () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Done",
    });

    const row = container.querySelector(".message-assistant");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    dispose();
  });

  it("does not animate user rows", () => {
    const { container, dispose } = renderMessage({
      from: "user",
      label: "You",
      content: "Hello",
    });

    const row = container.querySelector(".message-user");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    dispose();
  });
});
