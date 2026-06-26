## Summary

<!-- What changed and why -->

## Testing

- [ ] `./scripts/verify.sh`

## Release

Desktop app version: `crates/desktop/tauri.conf.json` (must match `crates/desktop/Cargo.toml` and `crates/desktop/package.json`).

CI runs `./scripts/check-version-bump.sh` and `./scripts/check-release-version.sh` on every PR.

### Shipping to users (desktop release)

Check this when the PR bumps the desktop app version:

- [ ] Desktop version bumped in `tauri.conf.json`, `Cargo.toml`, and `package.json`
- [ ] `crates/ui` version bumped if UI code changed
- [ ] Post-merge: `git tag vX.Y.Z && git push origin vX.Y.Z` (tag matches `tauri.conf.json`)
- [ ] Publish the draft GitHub Release after the Release workflow finishes

See [`docs/contributing/releasing.md`](docs/contributing/releasing.md).

### Not a user release

Leave unchecked if this PR is internal-only (no new build for users). Skip the post-merge tag.
