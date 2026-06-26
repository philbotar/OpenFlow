# Releasing OpenFlow (macOS)

User-facing builds ship through **GitHub Releases** and the in-app updater. Release steps are part of the PR workflow when you bump the desktop app version.

## Version files

Keep these in sync (CI enforces this):

| File | Role |
| --- | --- |
| `crates/desktop/tauri.conf.json` | **Canonical app version** — updater, GitHub tag, user-facing version |
| `crates/desktop/Cargo.toml` | Rust crate version (match `tauri.conf.json`) |
| `crates/desktop/package.json` | npm wrapper version (match `tauri.conf.json`) |

`crates/ui/package.json` is separate — bump it when UI code changes (`check-version-bump.sh`).

## PR checklist

1. Classify the change ([`development-lanes.md`](development-lanes.md)).
2. Run `./scripts/verify.sh`.
3. If users should receive the change, **bump the desktop app version** (patch for bug fixes).
4. If `crates/ui/**` changed, bump `crates/ui/package.json` too.
5. Open PR — CI runs version bump + release sync checks.
6. Merge.

## After merge (maintainer)

Only when the PR **bumped the desktop app version**:

```bash
git checkout main
git pull origin main
git tag v0.1.2          # must match tauri.conf.json version
git push origin v0.1.2
```

1. **Release** workflow builds signed macOS artifacts and opens a **draft** GitHub Release.
2. Review assets (`latest.json`, `.tar.gz`, `.dmg`).
3. **Publish** the release.

Installed apps check `https://github.com/philbotar/OpenFlow/releases/latest/download/latest.json`. Users see the blue **Settings** badge when the published version is newer.

## One-time setup

| Secret / config | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Signs updater bundles in CI |
| `plugins.updater.pubkey` in `tauri.conf.json` | Public half of signing key (in repo) |

Generate keys: `CI=1 ../ui/node_modules/.bin/tauri signer generate --write-keys ~/.tauri/openflow.key --force --ci` (from `crates/desktop`).

## Common cases

| Change | Desktop bump? | Tag after merge? |
| --- | --- | --- |
| Bug fix users should get | Yes (patch) | Yes |
| Engine/orchestration only, users should get fix | Yes — users only update the **desktop bundle** | Yes |
| UI-only, users should get change | Yes + bump `ui` crate | Yes |
| Docs, CI, internal refactor | No | No |
| WIP on `main` | No | No |

## Rules

- **Do not retag** a published version. Bump patch and push a new tag.
- **Draft releases** are invisible to the updater — publish when ready.
- Users on builds **without** the updater need one manual install first.
