import { execFile } from "node:child_process";
import { mkdir, rename, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import {
  emitMockEvent,
  getCapturedInvokes,
} from "@srsholmes/tauri-playwright";
import {
  expect,
  README_DEMO_PROMPT,
  README_DEMO_STATES,
  test,
} from "../fixtures.readme-demo.js";

const execFileAsync = promisify(execFile);
const FPS = 12;
const FRAME_DELAY_MS = Math.floor(1000 / FPS);
const EXPECTED_FRAMES = 16 * FPS;
const CHAT_DOCK_HEIGHT_RATIO = 0.7;
const e2eRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(e2eRoot, "../../..");

test.beforeEach(async ({ context }) => {
  await context.addInitScript(() => {
    localStorage.setItem("step-through-theme", "dark");
    localStorage.setItem("openflow.leftPanelHidden", "true");
    localStorage.setItem("openflow.rightPanelHidden", "true");
    localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
  });
});

test("capture README workflow demo", async ({ tauriPage }, testInfo) => {
  if (!("playwrightPage" in tauriPage)) {
    throw new Error("README capture requires the browser page adapter");
  }
  const page = tauriPage.playwrightPage;
  const framesDir = testInfo.outputPath("frames");
  const sourceDir = join(repoRoot, "test-results/readme-demo");
  const sourceVideo = join(sourceDir, "openflow-workflow-demo.mp4");
  const palette = join(sourceDir, "openflow-workflow-demo-palette.png");
  const gifPath = join(repoRoot, "docs/assets/openflow-workflow-demo.gif");
  const optimizedGif = join(sourceDir, "openflow-workflow-demo-optimized.gif");

  await mkdir(framesDir, { recursive: true });
  await mkdir(sourceDir, { recursive: true });
  await mkdir(dirname(gifPath), { recursive: true });

  await expect(
    page.getByRole("banner").getByText("Feature planning demo"),
  ).toBeVisible();
  await expect(page.locator(".workflow-flow-node")).toHaveCount(4);
  await expect(page.locator("textarea.composer-input")).toBeVisible();
  await resizeChatDock(page, CHAT_DOCK_HEIGHT_RATIO);
  await page.waitForTimeout(400);
  await centerGraph(page);
  await zoomGraphOutOneClick(page);
  await centerGraph(page);
  await expectGraphToFit(page, { horizontal: 64, vertical: 10 });
  await page.mouse.move(1368, 80);
  await expect(
    page.getByRole("tooltip", { name: "Zoom out" }),
  ).toBeHidden();

  let frame = 0;
  let cursor = { x: 1368, y: 80 };

  await page.evaluate(({ x, y }) => {
    const pointer = document.createElement("div");
    pointer.id = "readme-demo-pointer";
    pointer.setAttribute("aria-hidden", "true");
    Object.assign(pointer.style, {
      position: "fixed",
      left: `${x}px`,
      top: `${y}px`,
      width: "20px",
      height: "25px",
      zIndex: "2147483647",
      pointerEvents: "none",
      background: "#f8fafc",
      clipPath:
        "polygon(0 0, 0 100%, 29% 72%, 45% 100%, 58% 93%, 42% 67%, 100% 67%)",
      filter:
        "drop-shadow(0 0 1px #020617) drop-shadow(0 1px 2px rgba(2, 6, 23, 0.75))",
      transform: "translate(-2px, -2px)",
      transition: "filter 80ms ease",
    });
    document.body.append(pointer);
  }, cursor);

  const setCursor = async (x: number, y: number, clicking = false) => {
    cursor = { x, y };
    await page.mouse.move(x, y);
    await page.evaluate(
      ({ nextX, nextY, isClicking }) => {
        const pointer = document.getElementById("readme-demo-pointer");
        if (!pointer) return;
        pointer.style.left = `${nextX}px`;
        pointer.style.top = `${nextY}px`;
        pointer.style.filter = isClicking
          ? "drop-shadow(0 0 5px #60a5fa) drop-shadow(0 1px 2px rgba(2, 6, 23, 0.8))"
          : "drop-shadow(0 0 1px #020617) drop-shadow(0 1px 2px rgba(2, 6, 23, 0.75))";
      },
      { nextX: x, nextY: y, isClicking: clicking },
    );
  };

  const captureFrame = async () => {
    await page.waitForTimeout(FRAME_DELAY_MS);
    await page.screenshot({
      path: join(framesDir, `frame-${String(frame).padStart(4, "0")}.png`),
    });
    frame += 1;
  };

  const hold = async (count: number) => {
    for (let index = 0; index < count; index += 1) {
      await captureFrame();
    }
  };

  const moveCursorTo = async (
    locator: ReturnType<typeof page.locator>,
    count: number,
  ) => {
    const box = await locator.boundingBox();
    if (!box) throw new Error("Cannot move demo cursor to a hidden element");
    const target = {
      x: box.x + box.width / 2,
      y: box.y + box.height / 2,
    };
    const start = cursor;
    for (let index = 1; index <= count; index += 1) {
      const progress = index / count;
      await setCursor(
        start.x + (target.x - start.x) * progress,
        start.y + (target.y - start.y) * progress,
      );
      await captureFrame();
    }
  };

  const publishRunState = async (
    state: (typeof README_DEMO_STATES)[keyof typeof README_DEMO_STATES],
  ) => {
    await page.evaluate((nextState) => {
      Reflect.set(window, "__openflowReadmeDemoRunState", nextState);
    }, state);
    await emitMockEvent(page, "run-state", state);
  };

  const composer = page.locator("textarea.composer-input");
  const send = page.getByRole("button", {
    name: "Start workflow with message",
  });

  await hold(18);
  await moveCursorTo(composer, 4);
  await composer.click();
  for (let index = 1; index <= 18; index += 1) {
    const length = Math.ceil((README_DEMO_PROMPT.length * index) / 18);
    await composer.fill(README_DEMO_PROMPT.slice(0, length));
    await captureFrame();
  }
  await moveCursorTo(send, 4);
  await setCursor(cursor.x, cursor.y, true);
  await send.click();
  await expect
    .poll(async () =>
      (await getCapturedInvokes(page)).some((call) => call.cmd === "start_run"),
    )
    .toBe(true);
  await expect(
    page.getByRole("button", { name: "Stop workflow" }),
  ).toBeVisible();
  await publishRunState(README_DEMO_STATES.clarify);
  await expect(page.locator(".workflow-flow-node-started")).toHaveCount(1);
  await captureFrame();
  await setCursor(cursor.x, cursor.y);
  await hold(27);

  await publishRunState(README_DEMO_STATES.parallel);
  await expect(page.locator(".workflow-flow-node-started")).toHaveCount(2);
  await expect(page.getByText("2 agents are running in parallel.")).toBeVisible();
  await hold(48);

  await publishRunState(README_DEMO_STATES.final);
  await expect(page.locator(".workflow-flow-node-started")).toHaveCount(1);
  await expect(page.locator(".workflow-flow-node-completed")).toHaveCount(3);
  await hold(36);

  await publishRunState(README_DEMO_STATES.complete);
  await expect(page.locator(".workflow-flow-node-completed")).toHaveCount(4);
  await hold(4);
  const finalBriefFilter = page
    .getByRole("toolbar", { name: "Filter conversation by node" })
    .getByRole("button", { name: "Final brief", exact: true });
  await moveCursorTo(finalBriefFilter, 4);
  await setCursor(cursor.x, cursor.y, true);
  await finalBriefFilter.click();
  const finalBriefResponse = page.locator(
    '.chat-segment[data-node-id="brief"]',
  );
  await expect(finalBriefResponse).toContainText("Recommended approach");
  await expect(finalBriefResponse).toContainText(
    "Use command-based history around graph mutations",
  );
  await expect(finalBriefResponse).toContainText("Next step");
  await captureFrame();
  await setCursor(cursor.x, cursor.y);
  await captureFrame();
  await hold(26);

  expect(frame).toBe(EXPECTED_FRAMES);

  const framePattern = join(framesDir, "frame-%04d.png");
  await run("ffmpeg", [
    "-y",
    "-framerate",
    String(FPS),
    "-i",
    framePattern,
    "-vf",
    "scale=1280:-2:flags=lanczos",
    "-c:v",
    "libx264",
    "-preset",
    "medium",
    "-crf",
    "18",
    "-pix_fmt",
    "yuv420p",
    "-movflags",
    "+faststart",
    sourceVideo,
  ]);
  await run("ffmpeg", [
    "-y",
    "-i",
    sourceVideo,
    "-vf",
    "fps=12,scale=1280:-1:flags=lanczos,palettegen=max_colors=128:reserve_transparent=0",
    "-frames:v",
    "1",
    palette,
  ]);
  await run("ffmpeg", [
    "-y",
    "-i",
    sourceVideo,
    "-i",
    palette,
    "-lavfi",
    "[0:v]fps=12,scale=1280:-1:flags=lanczos[scaled];[scaled][1:v]paletteuse=dither=bayer:bayer_scale=3",
    "-loop",
    "0",
    gifPath,
  ]);

  if (await commandExists("gifsicle")) {
    await run("gifsicle", ["-O3", gifPath, "-o", optimizedGif]);
    await rename(optimizedGif, gifPath);
  }

  const gifStats = await stat(gifPath);
  expect(gifStats.size).toBeLessThan(10 * 1024 * 1024);
  console.log(`Source video: ${sourceVideo}`);
  console.log(`README GIF: ${gifPath}`);
  console.log(`GIF size: ${(gifStats.size / 1024 / 1024).toFixed(2)} MiB`);
});

async function resizeChatDock(
  page: import("@playwright/test").Page,
  viewportRatio: number,
) {
  const viewport = page.viewportSize();
  const resizeZone = page.getByRole("separator", {
    name: "Resize bottom panel",
  });
  const dock = page.locator(".dock-panel");
  const resizeBox = await resizeZone.boundingBox();
  const dockBox = await dock.boundingBox();
  if (!viewport || !resizeBox || !dockBox) {
    throw new Error("Cannot resize an unmeasured README demo chat dock");
  }

  const targetHeight = viewport.height * viewportRatio;
  const start = {
    x: resizeBox.x + resizeBox.width / 2,
    y: resizeBox.y + resizeBox.height / 2,
  };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x, start.y - (targetHeight - dockBox.height), {
    steps: 8,
  });
  await page.mouse.up();

  await expect
    .poll(async () => {
      const resizedDock = await dock.boundingBox();
      return resizedDock ? resizedDock.height / viewport.height : 0;
    })
    .toBeCloseTo(viewportRatio, 2);
}

async function centerGraph(page: import("@playwright/test").Page) {
  const canvas = page.locator(".react-flow");
  const nodes = page.locator(".react-flow__node");

  for (let attempt = 0; attempt < 2; attempt += 1) {
    const canvasBox = await canvas.boundingBox();
    const nodeBoxes = await nodes.evaluateAll((elements) =>
      elements.map((element) => {
        const box = element.getBoundingClientRect();
        return {
          left: box.left,
          top: box.top,
          right: box.right,
          bottom: box.bottom,
        };
      }),
    );
    if (!canvasBox || nodeBoxes.length === 0) {
      throw new Error("Cannot center an unmeasured README demo graph");
    }

    const graphBounds = {
      left: Math.min(...nodeBoxes.map((box) => box.left)),
      top: Math.min(...nodeBoxes.map((box) => box.top)),
      right: Math.max(...nodeBoxes.map((box) => box.right)),
      bottom: Math.max(...nodeBoxes.map((box) => box.bottom)),
    };
    const delta = {
      x:
        canvasBox.x +
        canvasBox.width / 2 -
        (graphBounds.left + graphBounds.right) / 2,
      y:
        canvasBox.y +
        canvasBox.height / 2 -
        (graphBounds.top + graphBounds.bottom) / 2,
    };
    if (Math.abs(delta.x) <= 2 && Math.abs(delta.y) <= 2) {
      return;
    }

    const start = {
      x: canvasBox.x + canvasBox.width - 32,
      y: canvasBox.y + canvasBox.height - 24,
    };
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + delta.x, start.y + delta.y, {
      steps: 8,
    });
    await page.mouse.up();
    await page.waitForTimeout(100);
  }

  throw new Error("README demo graph did not center after canvas panning");
}

async function zoomGraphOutOneClick(page: import("@playwright/test").Page) {
  const referenceNode = page.locator('.react-flow__node[data-id="idea"]');
  const before = await referenceNode.boundingBox();
  if (!before) {
    throw new Error("Cannot zoom out an unmeasured README demo graph");
  }

  await page.getByRole("button", { name: "Zoom out" }).click();
  await page.waitForTimeout(250);
  const after = await referenceNode.boundingBox();
  if (!after) {
    throw new Error("README demo graph disappeared after zooming out");
  }
  expect(after.width / before.width).toBeCloseTo(1 / 1.2, 2);
}

async function expectGraphToFit(
  page: import("@playwright/test").Page,
  margin: { horizontal: number; vertical: number },
) {
  const bounds = await measureGraph(page);
  expect(bounds.graph.left).toBeGreaterThanOrEqual(
    bounds.canvas.left + margin.horizontal,
  );
  expect(bounds.graph.top).toBeGreaterThanOrEqual(
    bounds.canvas.top + margin.vertical,
  );
  expect(bounds.graph.right).toBeLessThanOrEqual(
    bounds.canvas.right - margin.horizontal,
  );
  expect(bounds.graph.bottom).toBeLessThanOrEqual(
    bounds.canvas.bottom - margin.vertical,
  );
}

async function measureGraph(page: import("@playwright/test").Page) {
  return page.evaluate(() => {
    const canvas = document.querySelector(".react-flow");
    const nodes = Array.from(document.querySelectorAll(".react-flow__node"));
    if (!canvas || nodes.length === 0) {
      throw new Error("Cannot measure an empty README demo graph");
    }

    const canvasBox = canvas.getBoundingClientRect();
    const nodeBoxes = nodes.map((node) => node.getBoundingClientRect());
    return {
      canvas: {
        left: canvasBox.left,
        top: canvasBox.top,
        right: canvasBox.right,
        bottom: canvasBox.bottom,
      },
      graph: {
        left: Math.min(...nodeBoxes.map((box) => box.left)),
        top: Math.min(...nodeBoxes.map((box) => box.top)),
        right: Math.max(...nodeBoxes.map((box) => box.right)),
        bottom: Math.max(...nodeBoxes.map((box) => box.bottom)),
      },
    };
  });
}

async function run(command: string, args: string[]) {
  try {
    await execFileAsync(command, args, { maxBuffer: 10 * 1024 * 1024 });
  } catch (error) {
    const detail =
      error instanceof Error && "stderr" in error
        ? String(error.stderr)
        : String(error);
    throw new Error(`${command} failed:\n${detail}`);
  }
}

async function commandExists(command: string) {
  try {
    await execFileAsync(command, ["--version"]);
    return true;
  } catch (error) {
    if (
      error instanceof Error &&
      "code" in error &&
      error.code === "ENOENT"
    ) {
      return false;
    }
    throw error;
  }
}
