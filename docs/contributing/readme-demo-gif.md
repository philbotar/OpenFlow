# README workflow demo GIF

Record a request entering the workflow, two agents running in parallel, and a final agent combining their work. Keep the graph large enough to follow without reading the prompt text:

```text
idea -> clarify -> plan + risk (parallel) -> final brief
```

Use [`../../examples/feature_plan.workflow.json`](../../examples/feature_plan.workflow.json).
Do not build a new workflow during the recording. Keep graph editing out of this recording so the viewer can follow the run.

## Website video

The landing page now uses a real product screenshot; see [Product screenshots](documentation-site.md#product-screenshots) for capture provenance and asset handling. The earlier `docs/public/media/workflow-demo.mp4` and `workflow-demo-poster.png` remain available, and the README keeps the GIF for GitHub compatibility.

The earlier MP4 comes from the existing 1280 × 800, 12 fps GIF. It adds native playback controls and reduces the download size, but retains the GIF's limited detail. The repository's capture script, `crates/desktop/e2e/walkthroughs/readme-demo.capture.ts`, supplies scripted run events; the README labels that demo accordingly. Its original MP4 output, `test-results/readme-demo/openflow-workflow-demo.mp4`, is not included in the repository.

For a sharper replacement, record the current installed app at native resolution and at least 30 fps. Prefer a 2560 × 1600 capture, with the graph and conversation sized so labels remain readable at the website's 1120-pixel display width. Keep the original recording. Export the website video from that recording, never from the GIF.

After trimming the recording to the sequence below, export an H.264 MP4 with fast start and a still frame:

```bash
mkdir -p docs/public/media

ffmpeg -i openflow-demo.mov \
  -an -c:v libx264 -crf 18 -preset slow -pix_fmt yuv420p \
  -movflags +faststart docs/public/media/workflow-demo.mp4

ffmpeg -ss 7 -i docs/public/media/workflow-demo.mp4 \
  -frames:v 1 docs/public/media/workflow-demo-poster.png
```

Check the export before replacing either asset. The commands ask before overwriting an existing file. If you reintroduce video in `docs/.vitepress/theme/LandingPage.vue`, add native playback controls and captions that match the new recording. Keep the demo label accurate: only call it a real provider run if the capture actually uses a provider.

## Target

- Length: 14-18 seconds after trimming.
- Size: 1280 x 800, 12-15 FPS, under 10 MB.
- Layout: app window only; hide the desktop, menu bar, dock, terminal, and
  unrelated apps.
- Theme: dark, unless the README changes to a light visual style.
- Cursor: visible, moved deliberately, no circles or click effects.
- Authenticity: record a real provider run. Speed up waits in editing; do not
  fake node states or output.

## Prepare the app

1. Copy `examples/feature_plan.workflow.json` into a clean demo project's
   `.flow/workflows/` directory.
2. Open **Feature planning demo**.
3. Confirm all four nodes use an available, fast model.
4. Confirm provider readiness is green.
5. Run the workflow once off-camera. Check that:
   - **Clarify idea** finishes first.
   - **Create plan** and **Find risks** visibly run at the same time.
   - **Final brief** starts only after both parallel nodes finish.
   - The run completes without retries, approvals, or errors.
6. Return to a clean idle run.
7. Hide the left sidebar and inspector.
8. Keep the full graph centered on the canvas.
9. Open the bottom **Chat** panel to roughly 30% of the window height.
10. Close notifications. Remove any visible personal paths, project names,
    provider keys, unrelated chats, or prior output.
11. Set the window to 1440 x 900 or another 16:10 size. Record at native
    resolution.

Use this one-line starter message:

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

Keep the pointer away from node labels while agents run. The changing node
colors, status labels, animated edges, and parallel-agent message provide the
motion.

## Edit the capture

1. Remove startup, hesitation, accidental pointer movement, and the provider's
   long idle waits.
2. Keep at least one second of both middle nodes visibly running together.
3. Speed up only inactive waits. If the total run needs heavy compression, add
   a small `2x` label.
4. Hold the final trace frame for 1.5-2 seconds.
5. Add a 200-300 ms crossfade from the final frame back to the opening frame so
   the loop does not snap.
6. Crop to the app window. Preserve node labels and the readiness indicator.
7. Export the source recording too. A future UI change should not require
   screen-recording from scratch.

Example GIF export:

```bash
mkdir -p docs/assets

ffmpeg -i openflow-demo.mov \
  -vf "fps=12,scale=1280:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  -loop 0 docs/assets/openflow-workflow-demo.gif

# Optional final optimization.
gifsicle -O3 docs/assets/openflow-workflow-demo.gif \
  -o docs/assets/openflow-workflow-demo.gif
```

If the GIF exceeds 10 MB, reduce FPS to 10 first, then width to 1120. Do not
make text unreadable to preserve FPS.

## README placement

Store the final asset at:

```text
docs/assets/openflow-workflow-demo.gif
```

Place it after the overview text and before **Install**:

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

- Parallel execution is not visible for at least one second.
- Text, output, or project names contain personal or secret data.
- A retry, error, approval prompt, or stale run appears.
- The graph is clipped or the final node sits below the fold.
- The cursor darts between controls or covers labels.
- The final file exceeds 10 MB or labels blur at README width.
- The opening frame needs more than two seconds to explain itself.
