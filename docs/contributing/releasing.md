# Releasing OpenFlow (macOS)

User-facing builds ship through **GitHub Releases** and the in-app updater. **Merging to `main` does not publish a release** — only pushing a `v*` tag does.

## Release train (multiple PRs, one release)

Use this when you want several PRs on `main` before users get a new build.

```text
PR A (engine)  ──┐
PR B (ui)      ──┼──> main (no desktop version bump) ──> cut-release ──> tag vX.Y.Z ──> GitHub Release
PR C (desktop) ──┘
```

1. **Feature PRs** — bump only the library crates you touch (`engine`, `orchestration`, `providers`, `ui`). **Do not bump the desktop app version.**
2. **Merge as many PRs as you want** — CI passes without a tag or GitHub Release.
3. **When ready to ship** — on `main`, run `./scripts/cut-release.sh` (or open a small release PR with that output). This bumps the desktop app version once.
4. **Tag once** — `git tag vX.Y.Z && git push origin vX.Y.Z` (version must match `tauri.conf.json`).
5. **Publish** the draft GitHub Release after the Release workflow finishes.

`check-version-bump.sh` enforces library crate bumps per touched crate. It does **not** require a desktop app version bump on every PR — desktop version changes belong in the release cut step.

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
3. Bump library crate versions for every substantive crate you changed.
4. If `crates/ui/**` changed, bump `crates/ui/package.json`.
5. **Skip the desktop app version** unless this PR is explicitly the release cut (use `./scripts/cut-release.sh` on `main` instead).
6. Open PR — CI runs version bump + release sync checks.
7. Merge.

## After merge (maintainer)

Only when you are **cutting a release** (desktop app version was bumped):

```bash
git checkout main
git pull origin main
./scripts/cut-release.sh    # if not already bumped in a release PR
git tag v0.1.5              # must match tauri.conf.json version
git push origin v0.1.5
```

1. **Release** workflow builds signed macOS artifacts and opens a **draft** GitHub Release.
2. Review assets (`latest.json`, `.tar.gz`, `.dmg`).
3. **Publish** the release.

Installed apps check `https://github.com/philbotar/OpenFlow/releases/latest/download/latest.json`. Users see the blue **Settings** badge when the published version is newer.

## One-time setup

| Secret / config | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Signs updater bundles in CI |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Only if the key has a real password — **leave unset** for `--ci` keys (empty password) |
| `plugins.updater.pubkey` in `tauri.conf.json` | Public half of signing key (in repo) |

Generate keys and set the GitHub secret in one step (run on your machine — needs `gh` auth):

```bash
./scripts/setup-tauri-signing.sh
```

Manual equivalent (from `crates/desktop`):

`CI=1 ../ui/node_modules/.bin/tauri signer generate --write-keys ~/.tauri/openflow.key --force --ci`

Then `gh secret set TAURI_SIGNING_PRIVATE_KEY --repo philbotar/OpenFlow < ~/.tauri/openflow.key` and paste `openflow.key.pub` into `plugins.updater.pubkey` in `tauri.conf.json` (the script does both).

Validate before tagging:

```bash
./scripts/validate-tauri-signing.sh
```

If pubkey mismatch (secret already on GitHub): `./scripts/sync-tauri-pubkey.sh` then commit `tauri.conf.json`.

## Common cases

| Change | Library crate bump? | Desktop bump in PR? | Tag after merge? |
| --- | --- | --- | --- |
| Bug fix users should get | Yes (touched crates) | No — use release train | Only after `cut-release` |
| Engine/orchestration only | Yes | No | Only after `cut-release` |
| UI change | Yes (`ui`) | No | Only after `cut-release` |
| Desktop IPC / Tauri adapter | No (unless other crates touched) | No | Only after `cut-release` |
| Docs, CI, internal refactor | No | No | No |
| Ready to ship accumulated `main` | — | Yes (`./scripts/cut-release.sh`) | Yes |

## Rules

- **Merge ≠ release.** Multiple PRs can land on `main` without tagging.
- **Do not retag** a published version. Bump patch and push a new tag.
- **Draft releases** are invisible to the updater — publish when ready.
- Users on builds **without** the updater need one manual install first.
