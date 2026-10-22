# Build and test troubleshooting

Use these checks for a source checkout. For failures in the packaged app, use [Troubleshooting](../troubleshooting/README.md).

## Focused verification

When investigating a code failure, reproduce it with the narrowest relevant command before editing:

```bash
cargo nextest run -p providers
cargo nextest run -p orchestration --lib
cargo nextest run -p desktop
npm --prefix crates/ui run typecheck
./scripts/check-architecture.sh
```

Run `./scripts/verify.sh` for the canonical full gate. Provider fixture tests cannot prove a real account's Codex entitlement; record the interactive live smoke separately.

## MCP protocol checks

Run the deterministic local protocol lane:

```bash
./scripts/verify/mcp-live.sh
```

## Rust build disk usage

Cargo incremental compilation is disabled to keep workspace feature and test variants from growing `target/debug/incremental` without a bound. Compiled dependencies and sccache remain reusable.

Scripted Cargo entrypoints refuse to start when `target/debug` exceeds 64 GiB or free space falls
below 24 GiB locally (8 GiB on GitHub Actions). Override the limits with
`OPENFLOW_MAX_DEBUG_CACHE_GIB` and `OPENFLOW_MIN_BUILD_SPACE_GIB`; use `0` only to disable the
corresponding guard intentionally.

Delete only the rebuildable debug cache when the guard trips:

```bash
./scripts/clean-rust-cache.sh --yes
```

The command preserves source files, Git state, release artifacts, and cross-compiled
targets. The next Rust build is cold.
