#!/usr/bin/env node
import { mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(join(root, "crates/desktop/e2e/package.json"));
const { chromium } = require("@playwright/test");
const previewHtml = join(root, "tools/update-badge-preview.html");
const artifactsDir = process.env.CURSOR_ARTIFACTS_DIR ?? join(root, "artifacts");
const outputBase = join(artifactsDir, "update-badge-available");

async function capture(theme) {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 420, height: 260 } });
  await page.goto(pathToFileURL(previewHtml).href);
  await page.evaluate((nextTheme) => {
    document.documentElement.setAttribute("data-theme", nextTheme);
  }, theme);
  await page.waitForTimeout(100);
  const outputPath = `${outputBase}-${theme}.png`;
  await page.screenshot({ path: outputPath });
  await browser.close();
  return outputPath;
}

await mkdir(artifactsDir, { recursive: true });
const lightPath = await capture("light");
const darkPath = await capture("dark");
await writeFile(
  join(artifactsDir, "update-badge-manifest.txt"),
  `${lightPath}\n${darkPath}\n`,
);
console.log(lightPath);
console.log(darkPath);
