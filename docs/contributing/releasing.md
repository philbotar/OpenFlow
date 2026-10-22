# Releasing OpenFlow (macOS)

Pushing a `v*` tag starts the release workflow. Publishing its draft GitHub Release ships the build through Releases and the in-app updater. Merging to `main` alone releases nothing.

## Release train (multiple PRs, one release)

Merge feature PRs into `main`, then cut one desktop release for everything accumulated.

```text
PR A (engine)  ──┐
PR B (ui)      ──┼──> main (no desktop version bump) ──> cut-release ──> tag vX.Y.Z ──> GitHub Release
PR C (desktop) ──┘
```

1. In feature PRs, bump only the library crates you touch (`engine`, `orchestration`, `providers`, `ui`). **Do not bump the desktop app version.**
2. Merge after CI passes. No tag or GitHub Release needed.
3. On `main`, run `./scripts/cut-release.sh` (or open a small release PR with its output). This bumps the desktop app version once.
4. Tag it; the version must match `tauri.conf.json`: `git tag vX.Y.Z && git push origin vX.Y.Z`.
5. **Publish** the draft GitHub Release once the Release workflow finishes.

`check-version-bump.sh` requires a library version bump for each touched crate. The desktop version is bumped once, at release cut.

## Version files

CI enforces that these stay in sync:

| File | Role |
| --- | --- |
| `crates/desktop/tauri.conf.json` | **Canonical app version**: updater, GitHub tag, user-facing version |
| `crates/desktop/Cargo.toml` | Rust crate version (match `tauri.conf.json`) |
| `crates/desktop/package.json` | npm wrapper version (match `tauri.conf.json`) |

`crates/ui/package.json` is separate: bump it when UI code changes (`check-version-bump.sh`).

## PR checklist

1. Classify the change ([`development-lanes.md`](development-lanes.md)).
2. Run `./scripts/verify.sh` for code PRs and release cuts. Docs/instruction-only PRs use link, path, syntax, and diff checks.
3. Bump the library crate version of every substantive crate you changed.
4. If `crates/ui/**` changed, bump `crates/ui/package.json`.
5. **Skip the desktop app version** unless this PR is explicitly the release cut (otherwise use `./scripts/cut-release.sh` on `main`).
6. Open the PR. CI runs the version bump and release sync checks.
7. Merge.

## After merge (maintainer)

Only when **cutting a release** (desktop app version bumped):

```bash
git checkout main
git pull origin main
./scripts/cut-release.sh    # if not already bumped in a release PR
git tag v0.1.5              # must match tauri.conf.json version
git push origin v0.1.5
```

1. The **Release** workflow runs `release-verify` on Ubuntu first: `test-fast --execution --desktop` and `cargo nextest run -p orchestration --test workflow_e2e`. Only if it passes does the macOS build run, producing code-signed and updater-signed artifacts, verifying their signatures and architectures, and opening a **draft** GitHub Release.
2. Review the assets (`latest.json`, `.tar.gz`, `.dmg`).
3. **Publish** the release.

Installed apps check `https://github.com/philbotar/OpenFlow/releases/latest/download/latest.json` and show a blue **Settings** badge when the published version is newer.

### macOS release targets

One native bundle per Mac architecture:

| Mac | GitHub Actions runner | Rust target |
| --- | --- | --- |
| Apple Silicon (ARM64) | `macos-26` (Tahoe) | `aarch64-apple-darwin` |
| Intel | `macos-15-intel` (Sequoia) | `x86_64-apple-darwin` |

Explicit [runner labels](https://github.com/actions/runner-images#available-images) keep each build on its native architecture. The runner OS sets the build environment, not the app's minimum supported macOS. Rust caches use the workspace root `target/`, keyed per runner and Rust target. Both bundles pass signature and architecture checks before release review.

## Updater signing setup

| Secret / config | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Signs updater bundles in CI |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Only if the key has a real password: **leave unset** for `--ci` keys (empty password) |
| `plugins.updater.pubkey` in `tauri.conf.json` | Public half of signing key (in repo) |

Generate keys and set the GitHub secret in one step, on your machine (needs `gh` auth):

```bash
./scripts/setup-tauri-signing.sh
```

Manual equivalent, from `crates/desktop`:

```bash
CI=1 ../ui/node_modules/.bin/tauri signer generate --write-keys ~/.tauri/openflow.key --force --ci
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo philbotar/OpenFlow < ~/.tauri/openflow.key
```

Then paste `openflow.key.pub` into `plugins.updater.pubkey` in `tauri.conf.json`. (The script does both.)

Validate before tagging:

```bash
./scripts/validate-tauri-signing.sh
```

On a pubkey mismatch with a secret already on GitHub, run `./scripts/sync-tauri-pubkey.sh`, then commit `tauri.conf.json`.

Updater signatures protect update integrity. They don't satisfy macOS Gatekeeper.

## macOS code signing and notarization

`tauri.conf.json` sets `bundle.macOS.signingIdentity` to `-`, which makes a complete ad-hoc signature when Developer ID secrets are absent. Apple Silicon then won't call the downloaded app damaged, but users must still approve it in System Settings → Privacy & Security.

For public distribution, use a paid Apple Developer Program account. Create and export a **Developer ID Application** certificate, then add all of these repository secrets:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded exported `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: Name (TEAMID)` identity |
| `APPLE_ID` | Apple account email used for notarization |
| `APPLE_PASSWORD` | App-specific Apple account password |
| `APPLE_TEAM_ID` | Apple Developer team ID |
| `KEYCHAIN_PASSWORD` | Ephemeral CI keychain password |

With all secrets present, the Release workflow imports the certificate, overrides the ad-hoc identity, has Tauri notarize and staple the app, and requires Gatekeeper and stapler validation to pass. A partial secret setup fails before the build.

Before publishing a draft, download both DMGs and confirm:

```bash
./scripts/verify-macos-app.sh /path/to/OpenFlow.app arm64 developer-id
./scripts/verify-macos-app.sh /path/to/OpenFlow.app x86_64 developer-id
```

## Common cases

| Change | Library crate bump? | Desktop bump in PR? | Tag after merge? |
| --- | --- | --- | --- |
| Bug fix users should get | Yes (touched crates) | No: use release train | Only after `cut-release` |
| Engine/orchestration only | Yes | No | Only after `cut-release` |
| UI change | Yes (`ui`) | No | Only after `cut-release` |
| Desktop IPC / Tauri adapter | No (unless other crates touched) | No | Only after `cut-release` |
| Docs, CI, internal refactor | No | No | No |
| Ready to ship accumulated `main` | - | Yes (`./scripts/cut-release.sh`) | Yes |

Never retag a published version; bump the patch version and push a new tag. Drafts are invisible to the updater until published. Users on builds without an updater need one manual install first.
