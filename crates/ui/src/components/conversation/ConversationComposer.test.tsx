// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import type { PendingChatAttachment } from "../../lib/types";
import { ConversationComposer } from "./ConversationComposer";

const chatCss = readFileSync("src/styles/chat.css", "utf8");

afterEach(() => {
  document.body.replaceChildren();
});

function renderComposer(options: { active?: boolean; directChat?: boolean } = {}) {
  const [attachments, setAttachments] = createSignal<PendingChatAttachment[]>([]);
  const handlePickChatAttachments = vi.fn(async () => {
    setAttachments([
      {
        sourcePath: "/tmp/capture.png",
        fileName: "capture.png",
        kind: "image",
      },
    ]);
  });
  const handleRemovePendingChatAttachment = vi.fn(
    async (_nodeId: string, sourcePath: string) => {
      setAttachments((current) =>
        current.filter((attachment) => attachment.sourcePath !== sourcePath),
      );
    },
  );
  const handleStageChatAttachments = vi.fn(async () => undefined);
  const handleStopRun = vi.fn(async () => undefined);
  const context = {
    replayRunId: () => null,
    readiness: () => ({ ready: true }),
    runState: () => ({ active: options.active ?? false, pendingApprovals: [] }),
    activeWorkflow: () => ({ nodes: [], settings: {} }),
    activeChat: () => null,
    activeProfileMemo: () => ({
      known_models: [],
      default_model: null,
      reasoning_effort_options: [],
    }),
    projects: () => [],
    startingRun: () => false,
    chatDraft: () => "",
    setChatDraft: vi.fn(),
    availableSkills: () => [],
    skillById: () => new Map(),
    chatSubmissionFor: () => ({ submittedText: "", invokedSkills: [] }),
    searchProjectFileReferences: async () => [],
    handleChatInputKeyDown: vi.fn(),
    handleSubmitChat: vi.fn(),
    pendingChatAttachments: attachments,
    handlePickChatAttachments,
    handleRemovePendingChatAttachment,
    handleStageChatAttachments,
    canSendChatFor: () => attachments().length > 0,
    composerBusyFor: () => false,
    handleStopRun,
    stoppingRun: () => false,
  } as unknown as AppContextValue;
  const container = document.createElement("div");
  document.body.append(container);
  const dispose = render(
    () => (
      <AppContext.Provider value={context}>
        <ConversationComposer
          nodeId="__run_entry__"
          label="Chat"
          kickoff={!options.active}
          directChat={options.directChat}
        />
      </AppContext.Provider>
    ),
    container,
  );
  return {
    container,
    dispose,
    handlePickChatAttachments,
    handleRemovePendingChatAttachment,
    handleStageChatAttachments,
    handleStopRun,
  };
}

describe("ConversationComposer", () => {
  it("shows Stop after Send only for an active workflow", () => {
    const workflow = renderComposer({ active: true });
    const workflowSend = workflow.container.querySelector<HTMLButtonElement>(
      "button[aria-label='Send to paused node']",
    );
    const workflowStop = workflow.container.querySelector<HTMLButtonElement>(
      "button[aria-label='Stop workflow run']",
    );

    expect(workflowSend).not.toBeNull();
    expect(workflowStop).not.toBeNull();
    expect(
      workflowSend!.compareDocumentPosition(workflowStop!) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).not.toBe(0);
    workflowStop!.click();
    expect(workflow.handleStopRun).toHaveBeenCalledOnce();
    workflow.dispose();

    const directChat = renderComposer({ active: true, directChat: true });
    expect(
      directChat.container.querySelector("button[aria-label='Stop workflow run']"),
    ).toBeNull();
    directChat.dispose();
  });

  it("picks, displays, and removes an attachment-only draft", async () => {
    const view = renderComposer();
    const attach = view.container.querySelector<HTMLButtonElement>(
      "button[aria-label='Attach files']",
    );
    expect(attach).not.toBeNull();
    expect(
      view.container.querySelector<HTMLButtonElement>(
        "button[aria-label='Start workflow with message']",
      )?.disabled,
    ).toBe(true);

    attach!.click();
    await vi.waitFor(() => {
      expect(view.container.textContent).toContain("capture.png");
    });
    expect(view.handlePickChatAttachments).toHaveBeenCalledWith("__run_entry__");
    expect(
      view.container.querySelector<HTMLButtonElement>(
        "button[aria-label='Start workflow with message']",
      )?.disabled,
    ).toBe(false);

    view.container
      .querySelector<HTMLButtonElement>("button[aria-label='Remove capture.png']")
      ?.click();
    await vi.waitFor(() => {
      expect(view.container.textContent).not.toContain("capture.png");
    });
    expect(view.handleRemovePendingChatAttachment).toHaveBeenCalledWith(
      "__run_entry__",
      "/tmp/capture.png",
    );
    view.dispose();
  });

  it("stages pasted images through the shared file path", async () => {
    const view = renderComposer();
    const file = new File(["png"], "pasted.png", { type: "image/png" });
    const item = {
      kind: "file",
      type: "image/png",
      getAsFile: () => file,
    } as DataTransferItem;
    const event = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "clipboardData", {
      value: { items: [item] },
    });

    view.container.querySelector("textarea")?.dispatchEvent(event);

    await vi.waitFor(() => {
      expect(view.handleStageChatAttachments).toHaveBeenCalledWith(
        "__run_entry__",
        [file],
      );
    });
    view.dispose();
  });

  it("keeps attachment result announcements out of the visual layout", async () => {
    const style = document.createElement("style");
    style.textContent = chatCss;
    document.head.append(style);
    const view = renderComposer();

    try {
      view.container
        .querySelector<HTMLButtonElement>("button[aria-label='Attach files']")
        ?.click();

      const announcement = await vi.waitFor(() => {
        const element = view.container.querySelector<HTMLElement>(
          ".composer-attachment-announcement",
        );
        expect(element?.textContent).toBe("Added 1 attachment.");
        return element!;
      });
      const computed = getComputedStyle(announcement);
      expect(computed.position).toBe("absolute");
      expect(computed.width).toBe("1px");
      expect(computed.height).toBe("1px");
      expect(computed.overflow).toBe("hidden");
      expect(announcement.getAttribute("aria-live")).toBe("polite");
    } finally {
      view.dispose();
      style.remove();
    }
  });
});
