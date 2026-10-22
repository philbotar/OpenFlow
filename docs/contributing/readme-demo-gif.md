# README workflow demo GIF

Show a request entering the workflow, two agents running in parallel, and a final agent combining their work. Keep the graph large enough to follow without reading prompt text:

```text
idea -> clarify -> plan + risk (parallel) -> final brief
```

Use [`../../examples/feature_plan.workflow.json`](../../examples/feature_plan.workflow.json). Don't build or edit a workflow on camera; the viewer should follow the run.

## Website video

The landing page now uses a real product screenshot (see [Product screenshots](documentation-site.md#product-screenshots) for provenance and asset handling). The earlier `docs/public/media/workflow-demo.mp4` and `workflow-demo-poster.png` remain, and the README keeps the GIF for GitHub compatibility.

The earlier MP4 was made from the existing 1280 × 800, 12 fps GIF: native playback controls and a smaller download, but the GIF's limited detail. Its capture script, `crates/desktop/e2e/walkthroughs/readme-demo.capture.ts`, supplies scripted run events, and the README labels the demo that way. The script's original MP4 output, `test-results/readme-demo/openflow-workflow-demo.mp4`, isn't in the repository.

For a sharper replacement:

1. Record the current installed app at native resolution, at least 30 fps. Prefer 2560 × 1600, sized so graph and conversation labels stay readable at the site's 1120-pixel display width.
2. Keep the original recording. Export from it, never from the GIF.
3. Trim to the sequence below, then export an H.264 MP4 with fast start and a still frame:

   ```bash
   mkdir -p docs/public/media
   ffmpeg -i openflow-demo.mov \
     -an -c:v libx264 -crf 18 -preset slow -pix_fmt yuv420p \
     -movflags +faststart docs/public/media/workflow-demo.mp4
   ```

   ```bash
   ffmpeg -ss 7 -i docs/public/media/workflow-demo.mp4 \
     -frames:v 1 docs/public/media/workflow-demo-poster.png
   ```

4. Check the export before replacing either asset. The commands ask before overwriting.

If you bring video back to `docs/.vitepress/theme/LandingPage.vue`, add native playback controls and captions matching the new recording. Call it a real provider run only if the capture used a provider.

## Target

- Length: 14-18 seconds after trimming.
- Size: 1280 x 800, 12-15 FPS, under 10 MB.
- Layout: app window only; no desktop, menu bar, dock, terminal, or unrelated apps.
- Theme: dark, unless the README moves to a light style.
- Cursor: visible, deliberate, no circles or click effects.
- Authenticity: a real provider run. Speed up waits in editing; never fake node states or output.

## Prepare the app

1. Copy `examples/feature_plan.workflow.json` into a clean demo project's `.flow/workflows/`.
2. Open **Feature planning demo**.
3. Confirm all four nodes use an available, fast model.
4. Confirm provider readiness is green.
5. Run it once off-camera and check:
   - **Clarify idea** finishes first.
   - **Create plan** and **Find risks** visibly run at the same time.
   - **Final brief** starts only after both finish.
   - No retries, approvals, or errors.
6. Return to a clean idle run.
7. Hide the left sidebar and inspector.
8. Center the full graph on the canvas.
9. Open the bottom **Chat** panel to about 30% of window height.
10. Close notifications. Remove visible personal paths, project names, provider keys, unrelated chats, and prior output.
11. Set the window to 1440 x 900 or another 16:10 size. Record at native resolution.

Starter message:

```text
Plan undo and redo for a visual workflow editor.
```

## Record this take

| Time | Action | What the viewer learns |
| --- | --- | --- |
| 0.0-1.5 s | Hold on the centered four-node diamond. | OpenFlow is a visual workflow editor. |
| 1.5-3.5 s | Type or paste the starter message, then click the composer arrow. | A normal request starts the workflow. |
| 3.5-6.0 s | Hold while **Clarify idea** changes from running to completed. | Nodes expose live execution state. |
| 6.0-10.0 s | Hold long enough to show **Create plan** and **Find risks** running together. | Independent agents run in parallel. |
| 10.0-13.0 s | Show both parallel nodes complete, then **Final brief** run. | Downstream work waits for its dependencies. |
| 13.0-16.0 s | Let **Final brief** complete. Open **Run trace** and hold on the completed events. | The run produces inspectable output and trace history. |

Keep the pointer off node labels while agents run. Node colors, status labels, animated edges, and the parallel-agent message provide the motion.

## Edit the capture

1. Cut startup, hesitation, stray pointer movement, and long provider idle waits.
2. Keep at least one second of both middle nodes running together.
3. Speed up only inactive waits. If the run needs heavy compression, add a small `2x` label.
4. Hold the final trace frame 1.5-2 seconds.
5. Crossfade 200-300 ms from the last frame to the first so the loop doesn't snap.
6. Crop to the app window, keeping node labels and the readiness indicator.
7. Export the source recording too, so a future UI change doesn't force a full re-record.

Example GIF export:

```bash
mkdir -p docs/assets
ffmpeg -i openflow-demo.mov \
  -vf "fps=12,scale=1280:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  -loop 0 docs/assets/openflow-workflow-demo.gif
```

Optional final optimization:

```bash
gifsicle -O3 docs/assets/openflow-workflow-demo.gif \
  -o docs/assets/openflow-workflow-demo.gif
```

Over 10 MB? Drop to 10 FPS first, then to 1120 width. Never trade readable text for FPS.

## README placement

Store the final asset at:

```text
docs/assets/openflow-workflow-demo.gif
```

Place it after the overview text, before **Install**:

```html
<p align="center">
  <img
    src="docs/assets/openflow-workflow-demo.gif"
    alt="OpenFlow running a four-agent workflow with two agents executing in parallel"
    width="100%"
  />
</p>
<p align="center"><em>A real workflow run, shortened for display.</em></p>
```

## Reject and retake when

- Parallel execution shows for less than one second.
- Text, output, or project names contain personal or secret data.
- A retry, error, approval prompt, or stale run appears.
- The graph is clipped or the final node is below the fold.
- The cursor darts between controls or covers labels.
- The file exceeds 10 MB or labels blur at README width.
- The opening frame needs more than two seconds to explain itself.
