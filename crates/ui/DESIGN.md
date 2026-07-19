# OpenFlow UI — implementation pointer

**Canonical design spec:** [`../../DESIGN.md`](../../DESIGN.md) (repo root) — tokens in YAML frontmatter, six-section visual system, do's/don'ts.

**Machine-readable sidecar:** [`.impeccable/design.json`](../../.impeccable/design.json) — shadows, motion, component HTML/CSS for live panel.

## Where code lives

| Layer | Path |
| --- | --- |
| Tokens | `src/styles/tokens.css` |
| Global patterns | `src/styles/index.css`, `src/styles/chat.css` |
| Components | `src/components/` — import from `@/components` |

## Component quick map

| Component | Notes |
| --- | --- |
| `Button` | `variant`: `primary` \| `secondary` \| `danger`; `size`: `default` \| `small` \| `compact`; `ghost`, `stretch` |
| `ButtonRow` | Action groups; `align="end"` for modal footers |
| `SettingsSection` / `SectionHeader` | Settings card shell + eyebrow/title/description |
| `PanelEmptyState` | Empty panels |
| `TextSelect` | Custom select |
| `Spinner` | `sm` \| `md` |
| `CollapsibleSection` / `InspectorSection` | Inspector blocks |
| `AnimatedModal` / `PickerModal` | Modal shells |

```tsx
<Button variant="primary" onClick={save}>Save</Button>
<Button variant="secondary" ghost size="small">Cancel</Button>
```

## Conventions

- **SolidJS** for app UI; **React** only in workflow canvas (canvas keeps CSS button classes)
- New IPC: `api.ts` only; tests mock `api.ts`
- Sidebar lists: `SidebarNavButton`, `SidebarList`, `SidebarListRow`
- Theme: `data-theme="dark"` on `<html>`

## Intentionally not duplicated here

Form field markup (`text-input`, `checkbox-row`), onboarding CTAs (`of-btn-*`), topbar icon vocabulary — see `index.css` and root `DESIGN.md` Components section.
