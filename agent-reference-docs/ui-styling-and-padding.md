# UI Styling and Padding

Style source of truth for `crates/agent-workflow-app/src/ui/*`.

## Core Rule

Do not hard-code new spacing/color values in feature code first. Add or reuse tokens in `ui/theme.rs` (and module-local constants where layout-specific), then consume them.

## Icon System

All icons use **egui-phosphor** (`egui_phosphor::regular as ph`). Import it in every file that renders icons. Do not use raw Unicode glyphs (e.g. `"⚙"`, `"〈"`, `"✎"`) — they are font-dependent and render inconsistently.

Phosphor is registered as position 1 in the `Proportional` font family (see `main.rs`), so phosphor PUA chars render correctly with `egui::FontId::proportional(size)` — no special font family is needed.

### Approved icon map

| Purpose | Constant | Notes |
| --- | --- | --- |
| Sidebar collapse | `ph::CARET_LEFT` | Used in nav toggle button |
| Settings / gear | `ph::GEAR_SIX` | Used in nav settings row |
| Add / new | `ph::PLUS` | Used in nav new-workflow row and inspector |
| Rename / edit | `ph::PENCIL_SIMPLE` | Used in nav rename button |
| Confirm / check | `ph::CHECK` | Used in nav confirm and inspector |
| Delete | `ph::TRASH` | Used in inspector |
| Run / play | `ph::PLAY` | Used in inspector |
| Send message | `ph::ARROW_UP` | Used in the minimal chat composer send button |
| Link | `ph::LINK` | Used in inspector |

Add new icons only from `egui_phosphor::regular`. If you use a new icon, add it to this table.

## Icon + Label Layout (Nav Pills)

Icon and label text **must be drawn as separate painter calls** with `NAV_ICON_COL_W` between them. Never concatenate icon and text into one string — the gap becomes font-controlled and inconsistent.

```
pill_left + NAV_PILL_TEXT_X          → icon painter call
pill_left + NAV_PILL_TEXT_X + NAV_ICON_COL_W  → label painter call
```

Constants in `nav.rs`:

| Constant | Value | Purpose |
| --- | --- | --- |
| `NAV_PILL_TEXT_X` | `12.0` | Left inset of content within a pill |
| `NAV_ICON_COL_W` | `18.0` | Reserved width: icon glyph + gap to label |

For rows with no icon (workflow name list), text starts at `NAV_PILL_TEXT_X` directly.

## Padded Button Rows

Any button in a padded panel with `inner_margin: 0` needs an explicit `ui.add_space(NAV_PILL_INSET_X)` before it inside `ui.horizontal()`. Without it, the button renders flush against the panel edge.

```rust
ui.horizontal(|ui| {
    ui.add_space(NAV_PILL_INSET_X);  // align to pill left edge
    ui.add_sized([28.0, 28.0], egui::Button::new(...));
});
```

## Typography Tokens

Defined in `crates/agent-workflow-app/src/ui/theme.rs`.

| Token | Value |
| --- | --- |
| `TS_TITLE` | `13.0` |
| `TS_SECTION` | `11.0` |
| `TS_LABEL` | `10.0` |
| `TS_BODY` | `12.0` |

## Global Interaction Spacing

Defined in `crates/agent-workflow-app/src/ui/theme.rs` via `ctx.style_mut`.

| Token | Value |
| --- | --- |
| `style.spacing.button_padding` | `vec2(8.0, 4.0)` |
| `style.spacing.item_spacing` | `vec2(6.0, 4.0)` |
| `style.spacing.interact_size.y` | `24.0` |

## Color and Surface Tokens

Defined in `crates/agent-workflow-app/src/ui/theme.rs`.

| Token | Value |
| --- | --- |
| `SURFACE_0..3` | Base dark background stack |
| `BORDER` | Default border color |
| `ACCENT`, `ACCENT_DIM` | Primary action colors |
| `SUCCESS`, `DANGER` | State colors |
| `FLOATING_SURFACE`, `FLOATING_SURFACE_SOFT` | Floating inspector surfaces |
| `FLOATING_BORDER`, `FLOATING_RULE`, `FLOATING_SHADOW` | Floating inspector boundary/depth |
| `TEXT_BRIGHT`, `TEXT_DIM` | Text contrast levels |

## Baseline Radius

From current UI files:

| Usage | Radius |
| --- | --- |
| Inputs/buttons (default widgets) | `4-5` |
| Inline sections and framed controls | `6-10` |
| Floating inspector shell | `20` |
| Nav pills | `9` |
| Nav icon buttons | `8` |

## Baseline Padding and Gaps

Current values in `inspector.rs`, `nav.rs`, `settings.rs`, `widgets.rs`.

| Area | Token/Value |
| --- | --- |
| Inspector vertical rhythm | `INSPECTOR_GAP = 12.0` |
| Inspector section label gap | `FLAT_SECTION_LABEL_GAP = 6.0` |
| Inspector section bottom gap | `FLAT_SECTION_BOTTOM_GAP = 14.0` |
| Inspector shell inset | `Margin::symmetric(18, 16)` |
| Inspector text-edit inset | `Margin::symmetric(10, 9)` |
| Inspector row height | `ICON_BTN_SIZE = 30.0` |
| Nav top padding | `NAV_TOP_PADDING = 8.0` |
| Nav row height | `NAV_ROW_HEIGHT = 34.0` |
| Nav pill horizontal inset | `NAV_PILL_INSET_X = 6.0` |
| Nav pill vertical inset | `NAV_PILL_INSET_Y = 2.0` |
| Nav pill content left start | `NAV_PILL_TEXT_X = 12.0` |
| Nav icon column width | `NAV_ICON_COL_W = 18.0` |
| Field helper spacing | `ui.add_space(2.0)` then `ui.add_space(4.0)` |
| Settings header inset | `Margin::symmetric(24, 14)` |
| Settings cards inset | `Margin::same(16)` |

## Component Recipes

Use these patterns unless there is a deliberate exception:

1. Section blocks:
   - Label in `TS_LABEL` + `TEXT_DIM`
   - `6px` gap to control body
   - `14px` bottom gap before next section
2. Text-edit groups:
   - Frame fill `FLOATING_INPUT_BG` with border `FLOATING_INPUT_BORDER`
   - Radius `10`
   - Inner margin `10x9`
3. Action rows:
   - Fixed row height `30` (`ICON_BTN_SIZE`)
   - Primary button uses `ACCENT`; secondary actions use transparent or `FLOATING_SURFACE_SOFT`
4. Floating panel:
   - Width contract: target `340`, min `280`, right/top margin `20`
   - Outer shell radius `20`, inset `18x16`
5. Nav icon rows:
   - Icon at `pill_left + NAV_PILL_TEXT_X`, label at `pill_left + NAV_PILL_TEXT_X + NAV_ICON_COL_W`
   - Both use `FontId::proportional(TS_SECTION)` with phosphor PUA chars rendering via font fallback

## Do / Do Not

1. Do reuse existing constants before adding new ones.
2. Do keep spacing values on the existing scale (`2, 4, 6, 8, 10, 12, 14, 16, 18, 24, 28, 30, 34`).
3. Do keep status colors semantic (`SUCCESS`, `DANGER`, `ACCENT`), not ad-hoc.
4. Do not introduce one-off pixel values if an adjacent token already exists.
5. Do not bypass `theme::apply` for app-wide visual defaults.
6. Do not use raw Unicode glyphs for icons — always use `egui_phosphor::regular as ph`.
7. Do not concatenate icon + label text into one string — draw them separately with `NAV_ICON_COL_W` gap.
8. Do not place buttons flush against a zero-margin panel edge — always add `ui.add_space(NAV_PILL_INSET_X)` first.
