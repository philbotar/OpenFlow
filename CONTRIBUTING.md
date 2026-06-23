# Contributing

Start at [`docs/contributing/README.md`](docs/contributing/README.md) for development lanes, coding patterns, and testing workflows.

Before opening a pull request:

1. Classify your change with [`docs/contributing/development-lanes.md`](docs/contributing/development-lanes.md).
2. Run `./scripts/verify.sh` locally.
3. Update [`CHANGELOG.md`](CHANGELOG.md) for user-visible changes.
4. Bump the version in each touched crate's manifest when you change crate code (`./scripts/check-version-bump.sh` compares against `main`).

Architecture boundaries are enforced in CI via `./scripts/check-architecture.sh`. See [`docs/architecture/contract.md`](docs/architecture/contract.md).
