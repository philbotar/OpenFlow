// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { PendingToolApproval } from "../../lib/types";

const previewFileEdit = vi.hoisted(() => vi.fn());

vi.mock("../../api", () => ({
  previewFileEdit,
}));

import { ToolApprovalCardBody } from "./ToolApprovalCard";

const readApproval: PendingToolApproval = {
  approvalId: "approval-read",
  nodeId: "node-1",
  nodeLabel: "Research",
  toolCall: {
    id: "call-read",
    name: "read",
    arguments: { path: "README.md" },
  },
  tier: "read",
};

const editApproval: PendingToolApproval = {
  approvalId: "approval-edit",
  nodeId: "node-2",
  nodeLabel: "Editor",
  toolCall: {
    id: "call-edit",
    name: "edit",
    arguments: { path: "src/main.rs", patch: "..." },
  },
  tier: "write",
};

describe("ToolApprovalCardBody", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    previewFileEdit.mockReset();
    previewFileEdit.mockResolvedValue({
      entries: [
        {
          op: "update",
          path: "src/main.rs",
          diff: "+ added line",
        },
      ],
    });
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  function renderCard(
    approval: PendingToolApproval,
    onApprove = vi.fn(),
  ) {
    dispose = render(
      () => <ToolApprovalCardBody approval={approval} onApprove={onApprove} />,
      container,
    );
    return onApprove;
  }

  test("renders non-file tool arguments without preview request", async () => {
    renderCard(readApproval);

    expect(container.textContent).toContain("Read File");
    expect(container.textContent).toContain("README.md");
    expect(previewFileEdit).not.toHaveBeenCalled();
  });

  test("loads file edit preview and renders diff entries", async () => {
    renderCard(editApproval);

    await vi.waitFor(() => {
      expect(previewFileEdit).toHaveBeenCalledWith(
        "approval-edit",
        "edit",
        editApproval.toolCall.arguments,
      );
    });

    await vi.waitFor(() => {
      expect(container.textContent).toContain("src/main.rs");
      expect(container.textContent).toContain("+ added line");
    });
  });

  test("approve and deny buttons call onApprove", async () => {
    const onApprove = renderCard(editApproval);

    await vi.waitFor(() => {
      const approveButton = container.querySelector(".primary-button") as HTMLButtonElement;
      expect(approveButton.disabled).toBe(false);
    });

    (container.querySelector(".secondary-button") as HTMLButtonElement).click();
    expect(onApprove).toHaveBeenCalledWith(false);

    (container.querySelector(".primary-button") as HTMLButtonElement).click();
    expect(onApprove).toHaveBeenCalledWith(true);
  });

  test("shows preview warning when preview returns no entries", async () => {
    previewFileEdit.mockResolvedValue({ entries: [] });
    renderCard(editApproval);

    await vi.waitFor(() => {
      expect(container.textContent).toContain("Preview returned no diff");
    });
  });
});
