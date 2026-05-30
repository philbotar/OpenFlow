# Technology Stack

**Analysis Date:** 2026-05-30

## Languages

**Primary:**
- Rust (Edition 2021) — sole language across all crates

**Secondary:**
- None detected

## Runtime

**Environment:**
- Native desktop application via `eframe`/`egui`
- Tokio async runtime with multi-thread scheduler (`rt-multi-thread`)

**Package Manager:**
- Cargo
- Lockfile: `Cargo.lock` present at workspace root

**Toolchain:**
- Channel: `stable` (`rust-toolchain.toml`)
- Components: `rustfmt`, `clippy`

## Frameworks

**Core:**
- `eframe` 0.34.2 — Desktop windowing and event loop for native GUI app
- `egui` 0.34.2 — Immediate-mode GUI toolkit (canvas, widgets, panels)
- `egui-phosphor` 0.12.0 — Phosphor icon font integration for egui

**Async & Networking:**
- `tokio` 1.52.3 — Async runtime (`macros`, `rt-multi-thread`, `sync`)
- `reqwest` 0.13.3 — HTTP client with JSON and rustls TLS (`json`, `rustls` features)
- `futures` 0.3.32 — Async utilities and combinators
- `async-trait` 0.1.89 — Async trait support

**Serialization:**
- `serde` 1.0.228 — Serialization/deserialization derive macros
- `serde_json` 1.0.149 — JSON encoding/decoding

## Key Dependencies

**Critical:**
- `workflow-core` (internal) — Domain model, DAG validation, execution engine, AI port trait
- `openai-client` (internal) — OpenAI Responses API and Chat Completions API adapter

**Infrastructure:**
- `dirs` 6.0.0 — Cross-platform data-local directory resolution for persistence
- `uuid` 1.23.1 (`v4`, `serde`) — Workflow/node/edge ID generation
- `thiserror` 2.0.18 — Structured error enums with `#[derive(Error)]`

**Testing:**
- `wiremock` 0.6.5 — HTTP mock server for unit tests (openai-client tests)
- `tempfile` 3.27.0 — Temporary directories for file store tests

**Asset:**
- `Nunito-Regular.ttf` — Custom proportional font bundled in `crates/agent-workflow-app/assets/`

## Configuration

**Workspace Lints** (enforced in `Cargo.toml` and all crate `Cargo.toml` via `workspace = true`):
```toml
[workspace.lints.rust]
warnings = "deny"
unsafe_code = "forbid"

[workspace.lints.clippy]
all = "deny"
```

**Custom Cargo Alias** (`.cargo/config.toml`):
```toml
[alias]
clippy-max = "clippy --workspace --all-targets -- -D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo"
```

**Release Profile** (`Cargo.toml`):
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

**Environment:**
- `.env` file exists at repo root (not read for security)
- Key env vars expected:
  - `OPENAI_API_KEY` — Primary OpenAI authentication
  - `OPENAI_COMPATIBLE_API_KEY` — Fallback/compatible provider authentication

**Build:**
- No custom build scripts detected (`build.rs` not present)
- Standard `cargo build` / `cargo run` workflow

## Platform Requirements

**Development:**
- Rust stable toolchain with `rustfmt` and `clippy`
- macOS, Linux, or Windows (desktop GUI targets all three; macOS gets custom titlebar styling)

**Production:**
- Desktop-native binary (no containerization or web deployment)
- macOS receives special viewport treatment in `main.rs`:
  ```rust
  #[cfg(target_os = "macos")]
  {
      viewport = viewport
          .with_titlebar_shown(false)
          .with_fullsize_content_view(true)
          .with_title_shown(false);
  }
  ```

---

*Stack analysis: 2026-05-30*
