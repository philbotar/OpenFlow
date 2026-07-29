// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MessageAttachments } from "./MessageAttachments";

const loadChatAttachmentPreview = vi.hoisted(() => vi.fn());

vi.mock("../../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../api")>()),
  loadChatAttachmentPreview,
}));

afterEach(() => {
  document.body.replaceChildren();
  loadChatAttachmentPreview.mockReset();
});

describe("MessageAttachments", () => {
  it("loads bounded image previews by run and attachment ID", async () => {
    loadChatAttachmentPreview.mockResolvedValue({
      mediaType: "image/jpeg",
      dataBase64: "cHJldmlldw==",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const dispose = render(
      () => (
        <MessageAttachments
          runId="run-1"
          attachments={[
            {
              id: "attachment-1",
              fileName: "capture.png",
              mediaType: "image/png",
              sizeBytes: 2048,
              sha256: "abc",
              kind: "image",
            },
          ]}
        />
      ),
      container,
    );

    await vi.waitFor(() => {
      expect(container.querySelector("img")?.getAttribute("alt")).toBe("capture.png");
    });
    expect(loadChatAttachmentPreview).toHaveBeenCalledWith("run-1", "attachment-1");
    expect(container.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/jpeg;base64,cHJldmlldw==",
    );
    dispose();
  });

  it("renders documents as metadata-only cards", () => {
    const container = document.createElement("div");
    document.body.append(container);
    const dispose = render(
      () => (
        <MessageAttachments
          runId="run-1"
          attachments={[
            {
              id: "document-1",
              fileName: "notes.md",
              mediaType: "text/markdown",
              sizeBytes: 512,
              sha256: "def",
              kind: "document",
            },
          ]}
        />
      ),
      container,
    );

    expect(container.textContent).toContain("notes.md");
    expect(container.textContent).toContain("512 B");
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("iframe")).toBeNull();
    expect(container.querySelector("svg")).toBeNull();
    expect(loadChatAttachmentPreview).not.toHaveBeenCalled();
    dispose();
  });
});
