# OpenFlow

Rust desktop app for composing and running AI agent workflows.

## Install

```bash
./scripts/setup.sh
```

The script installs build prerequisites, compiles OpenFlow, and opens a disk image. Drag **OpenFlow** to **Applications**, then launch from there.

Unsigned local builds may be blocked on first launch. Right-click **OpenFlow** → **Open**, or run:

```bash
xattr -cr /Applications/OpenFlow.app
```

## Run from source

```bash
npm --prefix crates/desktop run start -- dev
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).
