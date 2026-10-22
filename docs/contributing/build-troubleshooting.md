# Build and test troubleshooting

For source checkouts. For packaged-app failures, see [Troubleshooting](../troubleshooting/README.md).

## Focused verification

Reproduce a code failure with the narrowest relevant command before editing:

```bash
cargo nextest run -p providers
cargo nextest run -p orchestration --lib
cargo nextest run -p desktop
npm --prefix crates/ui run typecheck
./scripts/check-architecture.sh
```

`./scripts/verify.sh` is the canonical full gate. Provider fixture tests can't prove a real account's Codex entitlement; record the interactive live smoke separately.

## MCP protocol checks

Deterministic local protocol lane:

```bash
./scripts/verify/mcp-live.sh
```

## Rust build disk usage

Cargo incremental compilation is off, so workspace feature and test variants can't grow `target/debug/incremental` without bound. Compiled dependencies and sccache stay reusable.

Scripted Cargo entrypoints refuse to start when `target/debug` exceeds 64 GiB or free space drops below 24 GiB locally (8 GiB on GitHub Actions). Override with `OPENFLOW_MAX_DEBUG_CACHE_GIB` and `OPENFLOW_MIN_BUILD_SPACE_GIB`; `0` disables that guard, so use it only on purpose.

When a guard trips, delete only the rebuildable debug cache:

```bash
./scripts/clean-rust-cache.sh --yes
```

Source files, Git state, release artifacts, and cross-compiled targets are kept. The next Rust build is cold.
