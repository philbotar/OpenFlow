# Documentation website

Edit the Markdown in `docs/`. VitePress builds the website and its local search index from it. Relative links work on GitHub too; the build resolves them for the site.

Install [Bun](https://bun.sh/docs/installation) 1.3.14 and Node.js 22. From the repository root:

```bash
cd docs
bun install --frozen-lockfile
bun run dev
```

Open the local URL VitePress prints. To check the production build:

```bash
bun run typecheck
bun run build
bun run check
bun run preview
```

- `typecheck` checks the VitePress config.
- `check` verifies every internal link, image, and anchor in the built site. Run it after `build`.
- Links to repository files outside `docs/`, or to unpublished pages, open the file on GitHub.

`DOCS_SITE_URL` sets the site URL. Default: `https://philbotar.github.io/OpenFlow/`. The build uses its path as the site base (`/OpenFlow/` by default) and the full URL in the sitemap. To host at a domain root, set the root:

```bash
DOCS_SITE_URL=https://docs.example.com/ bun run build
```

## Pages and navigation

- [The landing page source](../index.md) is the site home page.
- [OpenFlow overview](../README.md) is the GitHub entry point; on the site it's `overview.html`.
- Other `README.md` pages become directory indexes: `getting-started/README.md` → `getting-started/`.

Add new guides to [the docs index](../README.md) and the sidebar in `docs/.vitepress/config.ts`. The build includes every Markdown page except `AGENTS.md`, `CONTEXT.md`, `ROADMAP.md`, and `work-specs/`. Links to those, and to repository source, open GitHub.

Use relative links to existing files. Keep commands in fenced code blocks. Mermaid fences render as diagrams that follow the light/dark theme; if rendering fails, the source shows. The site uses the OpenFlow app icon on the default VitePress theme (navigation, local search, light/dark controls).

## Product screenshots

Store captures in `docs/public/media/screenshots/`. From a guide, use a relative image path such as `../public/media/screenshots/release-checklist-inspector.png`, and link the image to the same file for full-size viewing. These paths render on GitHub. The site maps public assets under `/OpenFlow/media/`; Markdown exports use the absolute deployed image URL. Components use VitePress `withBase()` for asset URLs.

Place each screenshot next to the action it explains, with descriptive alt text and a short caption. The `product-shot` figure style fits the guide column; the full-size link keeps it readable on small screens. Keep prompt text and instructions out of the image. Use diagrams for dependency semantics, screenshots for real controls and states.

Current PNGs: installed OpenFlow 0.1.23, macOS, 7 September 2026.

- Appshots inspected and arranged the app, but its 1264 × 768 compressed JPEGs were too soft for the landing page.
- Window-only macOS `screencapture` produced the lossless 2880 × 1750 originals, unresized and unaltered: more than 2 source pixels per CSS pixel at the landing image's 1120px maximum width.
- Check text and composition, not just dimensions; a large image can still be blurry.

| File | Shows |
| --- | --- |
| `offline-search-canvas.png` | The prepared five-node branch-and-join workflow, before execution |
| `release-checklist-inspector.png`, `release-checklist-run.png` | The previously approved, completed tutorial run |

Sidebars are hidden to exclude personal data. The tutorial keeps its two-agent scope; the landing showcase shows a richer dependency graph. The tutorial crops the Inspector with CSS at its native logical width; opening the image shows the full original.

### Guide capture inventory

Also from installed OpenFlow 0.1.23, 7 September 2026:

1. Appshots/computer use inspected and arranged the real app.
2. Window-only macOS `screencapture -x -o -l WINDOW_ID -t png` captured fresh 2880 × 1750 PNGs, with the window ID discovered for that session.
3. `sips` checked PNG format and pixel dimensions right after capture and cut detail regions without resampling.

No compressed previews, generated UI, or upscaling. Files are under `docs/public/media/screenshots/`. Maximum CSS widths keep at least two source pixels per displayed pixel. Every image links to its original: most to the full native window, the two privacy crops to the full retained detail (the surrounding window held unrelated user data and isn't published).

| Guide / state | Detail PNG | Pixels / maximum CSS width | Full-window original |
| --- | --- | --- | --- |
| Workflow authoring, prepared branches | `offline-search-branches-detail.png` | 1450 × 650 / 700px | `offline-search-authoring-inspector.png` |
| Workflow authoring, selected node | `offline-search-inspector-detail.png` | 576 × 1270 / 288px | `offline-search-authoring-inspector.png` |
| Direct chat, unsent request | `direct-chat-composer-detail.png` | 1250 × 190 / 625px | `direct-chat-prepared.png` |
| Project chat, unsent request | `project-chat-composer-detail.png` | 1250 × 190 / 625px | `project-chat-prepared.png` |
| History, earlier completed tutorial run | `release-checklist-history-detail.png` | 850 × 160 / 425px | `release-checklist-history.png` |
| Scheduling, disabled demo | `schedule-time-and-days.png` | 700 × 520 / 350px | Privacy crop: complete dialog retained |
| Help, final guided-tour step | `help-tour-replay.png` | 1110 × 430 / 555px | Privacy crop: Help and complete tour card retained |

During capture, the branching workflow wasn't run and the prepared chat wasn't sent. The disposable `OpenFlow-docs-demo` project lived in temporary storage. The demo schedule was saved Off with Next: Never. No new model requests started.

The History capture shows a real stored `completed` status, not a verified good deliverable: that tutorial conversation had a completion summary plus run-review concerns about its output. Don't cite it as evidence of a substantive final checklist.

The review covered installation and provider setup, first workflow, authoring and Inspector, direct chats, runs and recovery, projects, scheduling, app navigation, and tools/integrations. The tutorial step rail and diagrams stay. Provider authentication stays text-only so no credentials or account details are published. Recovery keeps its decision diagram instead of a fabricated approval, failure, or resumed state. The Help capture shows the replayable tour, not fresh-install onboarding.

## Search and LLM output

Each build also writes these files into `docs/.vitepress/dist/`:

| Output | Content |
| --- | --- |
| `llms.txt` | Site summary and a link plus description for every page |
| `llms-full.txt` | All published pages in one file |
| One `.md` file per page | Page Markdown with links rewritten to absolute site URLs |
| `sitemap.xml` | Published page URLs with last-modified dates from git |

Every page gets its own meta description, taken from its front matter `description` or its first paragraph. Pages also get a canonical URL, Open Graph tags with a share image, and a link to their Markdown copy. The home page adds `SoftwareApplication` structured data. Set a front matter `description` to override the generated one. The code is in `docs/.vitepress/seo.ts`.

The repository's Pages site can't serve `robots.txt`, because that file must sit at the host root. Submit `sitemap.xml` in a search console instead.

## Publish

`bun run build` outputs a static directory, `docs/.vitepress/dist/`. It needs no OpenFlow backend or model credentials; any static host that serves directory indexes works.

### GitHub Pages

The [documentation workflow](../../.github/workflows/docs.yml) type-checks, builds, and runs `bun run check` on pull requests. On `main` it also deploys to GitHub Pages. You can run it manually from the Actions tab.

1. In **Settings → Pages**, set **Source** to **GitHub Actions**.
2. Once the workflow and docs reach `main`, the site is at `https://philbotar.github.io/OpenFlow/`.

For a custom domain, configure it in **Settings → Pages**, add the DNS records GitHub lists, and set the repository Actions variable `DOCS_SITE_URL` to the full public URL, with path and trailing slash.

### Other static hosts

Build with `DOCS_SITE_URL` set to the host's public URL, then upload the contents of `docs/.vitepress/dist/`. Rebuild whenever the URL changes so asset paths and the sitemap match.
