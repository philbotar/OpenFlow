# Documentation website

Edit the Markdown in `docs/`. VitePress builds the website and its local search index from those files. GitHub keeps using the same relative Markdown links; the website resolves them during the build.

Install [Bun](https://bun.sh/docs/installation) 1.3.14 and Node.js 22, then run these commands from the repository root:

```bash
cd docs
bun install --frozen-lockfile
bun run dev
```

Open the local URL printed by VitePress. To check the production build:

```bash
bun run typecheck
bun run build
bun run preview
```

The site path comes from `DOCS_SITE_URL`, which defaults to `https://philbotar.github.io/OpenFlow/`. The build uses that URL's path as the site base, so the default is `/OpenFlow/`, and it uses the full URL in the sitemap. For hosting at a domain root, set the variable to that root, for example `DOCS_SITE_URL=https://docs.example.com/ bun run build`. `typecheck` checks the VitePress config; the build does not validate links to files outside `docs/`.

## Pages and navigation

[The landing page source](../index.md) supplies the website home page. [OpenFlow overview](../README.md) is the product entry point for GitHub and appears at `overview.html` on the site. Other `README.md` pages become directory indexes: `getting-started/README.md` becomes `getting-started/`.

Add new guides to [the docs index](../README.md) and to the sidebar in `docs/.vitepress/config.ts`. The build includes Markdown pages automatically, except `AGENTS.md`, `CONTEXT.md`, `ROADMAP.md`, and files under `work-specs/`. Links to those files, repository source, and standalone diagram files do not resolve on the website.

Use relative links to existing files and keep commands inside fenced code blocks. Mermaid fences render as code blocks on the website. The site uses the OpenFlow app icon with the default VitePress theme: navigation, local search, and light/dark controls.

## Product screenshots

Store product captures in `docs/public/media/screenshots/`. Use a relative Markdown image destination such as `../public/media/screenshots/release-checklist-inspector.png` from a guide. Link the image to the same file so readers can open it at full size. These paths render on GitHub; the website maps public assets under `/OpenFlow/media/`, and Markdown exports use the absolute deployed image URL. Components use VitePress `withBase()` for asset URLs.

The current PNG captures were taken from installed OpenFlow 0.1.23 on macOS on 7 September 2026. Appshots was used to inspect and arrange the app, but its 1264 × 768 compressed JPEG output was too soft for the landing page. Window-only macOS `screencapture` produced the original lossless 2880 × 1750 PNGs used here, without resizing or UI alterations. That provides more than 2 source pixels per CSS pixel at the landing image’s 1120px maximum width. Inspect text and composition as well as dimensions; a large image can still be blurry.

`offline-search-canvas.png` shows the prepared five-node branch-and-join workflow before execution. `release-checklist-inspector.png` and `release-checklist-run.png` show the previously approved, completed tutorial run. Sidebars are hidden to exclude personal data. The tutorial keeps its two-agent scope; the landing showcase demonstrates a richer dependency graph. The tutorial frames the Inspector portion with CSS at its native logical width; opening the image retains the complete original.

Keep screenshots next to the action they explain, with descriptive alt text and a short caption. The `product-shot` figure style fits the guide column; a full-size link preserves readability on small screens. Keep prompt text and instructions outside the image, too. Use diagrams for dependency semantics, and screenshots for actual controls and states.

### Guide capture inventory

The additional guide captures below were taken on 7 September 2026 from installed OpenFlow 0.1.23. Appshots/computer use inspected and arranged the real app; window-only macOS `screencapture -x -o -l WINDOW_ID -t png` captured fresh 2880 × 1750 PNGs. The current window ID was discovered for this session. `sips` checked PNG format and pixel dimensions immediately after capture and extracted detail regions without resampling. No compressed previews, generated UI, or upscaling were used.

All filenames below are under `docs/public/media/screenshots/`. The maximum CSS widths preserve at least two source pixels per displayed pixel. Each image is also a link: most open the complete native window; the two privacy crops open the complete retained detail because the surrounding window contained unrelated user data and is not published.

| Guide / state | Detail PNG | Pixels / maximum CSS width | Full-window original |
| --- | --- | --- | --- |
| Workflow authoring, prepared branches | `offline-search-branches-detail.png` | 1450 × 650 / 700px | `offline-search-authoring-inspector.png` |
| Workflow authoring, selected node | `offline-search-inspector-detail.png` | 576 × 1270 / 288px | `offline-search-authoring-inspector.png` |
| Direct chat, unsent request | `direct-chat-composer-detail.png` | 1250 × 190 / 625px | `direct-chat-prepared.png` |
| Project chat, unsent request | `project-chat-composer-detail.png` | 1250 × 190 / 625px | `project-chat-prepared.png` |
| History, earlier completed tutorial run | `release-checklist-history-detail.png` | 850 × 160 / 425px | `release-checklist-history.png` |
| Scheduling, disabled demo | `schedule-time-and-days.png` | 700 × 520 / 350px | Privacy crop: complete dialog retained |
| Help, final guided-tour step | `help-tour-replay.png` | 1110 × 430 / 555px | Privacy crop: Help and complete tour card retained |

The branching workflow was not executed. The prepared chat was not sent. The disposable `OpenFlow-docs-demo` project was registered in temporary storage, and the demo schedule was saved Off with Next: Never. No new model requests were started. The History capture documents a real stored `completed` status, not a verified successful deliverable: the earlier tutorial conversation included a completion summary and run-review concerns about its output. Do not reuse it as evidence of a substantive final checklist.

The review covered installation and provider setup, first workflow, authoring and Inspector, direct chats, runs and recovery, projects, scheduling, app navigation, and tools/integrations. The existing tutorial step rail and diagrams remain. Provider authentication stays in text to avoid publishing configured credentials or account details; recovery retains its decision diagram rather than fabricating an approval, failure, or resumed state. The Help capture shows the replayable tour, not a fresh-install onboarding state.

## Publish

The build output is a static directory, `docs/.vitepress/dist/`. No OpenFlow backend or model credentials are required, and any static host that serves directory indexes can serve it.

### GitHub Pages

The [documentation workflow](../../.github/workflows/docs.yml) type-checks and builds on pull requests. On `main`, it also deploys the generated site to GitHub Pages. It can run manually from the Actions tab.

In the repository's **Settings → Pages**, set **Source** to **GitHub Actions**. After the workflow and documentation changes reach `main`, the default address is `https://philbotar.github.io/OpenFlow/`. For a custom domain, configure it in **Settings → Pages**, add the DNS records GitHub specifies, and set the repository Actions variable `DOCS_SITE_URL` to the full public URL, including its path and trailing slash.

### Other static hosts

Build with `DOCS_SITE_URL` set to the host's public URL, then upload the contents of `docs/.vitepress/dist/`. Rebuild whenever the URL changes so asset paths and the sitemap match.
