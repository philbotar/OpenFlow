import { test, expect } from "../fixtures.attachments.js";

type RecordedCall = {
  type: string;
  args?: Record<string, unknown>;
};

async function recordedCalls(
  page: { evaluate(script: string): Promise<unknown> },
): Promise<RecordedCall[]> {
  return (await page.evaluate(
    "window.__openflowE2e?.calls ?? []",
  )) as RecordedCall[];
}

test.describe("chat attachments", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
      (
        window as unknown as {
          __openflowAttachmentScenario?: string;
        }
      ).__openflowAttachmentScenario = "success";
    });
  });

  test("sends an attachment-only direct chat and reopens its preview", async ({
    tauriPage,
  }) => {
    await tauriPage.waitForSelector(".sidebar", 15_000);
    await tauriPage.getByRole("button", { name: "Attachment chat", exact: true }).click();

    const attach = tauriPage.getByRole("button", { name: "Attach files", exact: true });
    await attach.click();
    await expect(tauriPage.locator(".composer-attachment-card")).toContainText(
      "capture.png",
    );

    const send = tauriPage.getByRole("button", { name: "Send message", exact: true });
    await expect(send).toBeEnabled();
    await send.click();

    await expect(tauriPage.locator(".composer-attachment-card")).toHaveCount(0);
    const preview = tauriPage.getByAltText("capture.png");
    await expect(preview).toHaveAttribute(
      "src",
      /^data:image\/jpeg;base64,/,
    );

    expect(await recordedCalls(tauriPage)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: "start_chat",
          args: expect.objectContaining({
            chatId: "chat-attachments",
            message: {
              text: "",
              attachmentSourcePaths: ["/tmp/capture.png"],
            },
          }),
        }),
        expect.objectContaining({
          type: "load_chat_attachment_preview",
          args: {
            runId: "run-attachments",
            attachmentId: "attachment-image-1",
          },
        }),
      ]),
    );

    await tauriPage.getByRole("button", { name: "Other chat", exact: true }).click();
    await tauriPage.getByRole("button", { name: "capture.png", exact: true }).click();

    await expect(tauriPage.getByAltText("capture.png")).toHaveAttribute(
      "src",
      /^data:image\/jpeg;base64,/,
    );
    expect(await recordedCalls(tauriPage)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: "replay_run",
          args: { runId: "run-attachments" },
        }),
      ]),
    );
  });

  test("retains a pending card and shows the import error after rejection", async ({
    tauriPage,
  }) => {
    await tauriPage.waitForSelector(".sidebar", 15_000);
    await tauriPage.evaluate(
      'window.__openflowAttachmentScenario = "failure"',
    );
    await tauriPage.getByRole("button", { name: "Attachment chat", exact: true }).click();
    await tauriPage.getByRole("button", { name: "Attach files", exact: true }).click();

    const pending = tauriPage.locator(".composer-attachment-card");
    await expect(pending).toContainText("rejected.png");
    await tauriPage.getByRole("button", { name: "Send message", exact: true }).click();

    await expect(
      tauriPage.getByText("Attachment import failed: invalid image data."),
    ).toBeVisible();
    await expect(pending).toContainText("rejected.png");
  });
});
