# Contributing

Start at [`docs/contributing/README.md`](docs/contributing/README.md) for development lanes, coding patterns, and testing workflows.

Before opening a pull request:

1. Classify your change with [`docs/contributing/development-lanes.md`](docs/contributing/development-lanes.md).
2. Run `./scripts/verify.sh` locally.
3. Update [`CHANGELOG.md`](CHANGELOG.md) for user-visible changes when that file exists.
4. Bump the version in each touched crate's manifest when you change crate code (`./scripts/check-version-bump.sh` compares against `main`).
5. If you ship a **desktop release**, keep `tauri.conf.json`, `crates/desktop/Cargo.toml`, and `crates/desktop/package.json` on the same version, then tag after merge — see [`docs/contributing/releasing.md`](docs/contributing/releasing.md).

Architecture boundaries are enforced in CI via `./scripts/check-architecture.sh`. See [`docs/architecture/contract.md`](docs/architecture/contract.md).
