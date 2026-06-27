## Summary

<!-- What changed and why -->

## Testing

- [ ] `./scripts/verify.sh`

## Release

Desktop app version lives in `crates/desktop/tauri.conf.json` (must match `crates/desktop/Cargo.toml` and `crates/desktop/package.json`).

CI runs `./scripts/check-version-bump.sh` and `./scripts/check-release-version.sh` on every PR.

### Feature PR (default)

Most PRs should **not** bump the desktop app version. Merge to `main` without tagging.

- [ ] Bumped library crate versions for every substantive crate you changed (`engine`, `orchestration`, `providers`, `ui`)
- [ ] `crates/ui/package.json` bumped if UI code changed
- [ ] Desktop app version **unchanged** (release train — see below)

### Release cut (maintainer, when ready to ship)

Check this only when cutting a release after one or more feature PRs merged:

- [ ] Ran `./scripts/cut-release.sh` on `main` (or this PR only bumps desktop version via that script)
- [ ] Post-merge: `git tag vX.Y.Z && git push origin vX.Y.Z` (tag matches `tauri.conf.json`)
- [ ] Publish the draft GitHub Release after the Release workflow finishes

See [`docs/contributing/releasing.md`](docs/contributing/releasing.md) — **Release train** section.
