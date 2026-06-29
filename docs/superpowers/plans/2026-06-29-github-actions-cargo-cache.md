# CI Time Improvement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut PR CI wall-clock and runner minutes. Critical path today ~9–10 min (`build` ~4–5 min → `test` ~4.5 min); Miri adds ~3–5 min per crate on engine/orchestration PRs (parallel but billable). Target after Tiers 1–3: ~7–8 min critical path, no Miri jobs on PRs.

**Architecture:** Cargo build artifacts, `~/.cargo` registry+git, rustup toolchain, npm deps, and the Miri sysroot are **already** cached. This plan: (1) caches apt Tauri deps with first-party `actions/cache@v4`; (2) dedupes `ui-typecheck` (run once, in the `ui` job); (3) moves Miri + deep checks (`workflow_e2e`, `--desktop`) off PR CI into a `release-verify` job in `release.yml` that gates the macOS build on tag push (`push: tags: v*` + `workflow_dispatch`, **not** `release: types: [published]`); (4) caches the `taiki-e` cargo tools in `lint-extras`; (5) path-filters Rust/ui jobs off on docs-only PRs; (6, optional) `cargo-nextest` + a parallel test matrix, gated on measurement.

**Tech Stack:** GitHub Actions, `actions/cache@v4` (first-party), `swatinem/rust-cache@v2` (already in use), `taiki-e/install-action@v2` (already in use), `dorny/paths-filter@v3`, `cargo-nextest`, Miri.

---

## Already covered — DO NOT re-cache

| What | Already cached by | Where |
| --- | --- | --- |
| `./target` build artifacts | `swatinem/rust-cache@v2` (shared-key `ci-verify`, `cache-on-failure: true`) | `ci.yml:68-72`, `87-91`, `110-114`, `140-144`, `211-215`; `release.yml:44-47` |
| `~/.cargo` (registry + git + bin) | `swatinem/rust-cache@v2` (caches `~/.cargo` by default; skips `registry/src`) | same steps |
| rustup toolchain | `actions/cache@v4` keyed on `rust-toolchain.toml` + `Cargo.lock` | `.github/actions/setup-rust/action.yml:17-26` |
| npm deps | `actions/setup-node@v4` `cache: npm` | `ci.yml:58-63` etc., `release.yml:29-33` |

## Tier 4 — DO NOT (decided out of scope)

| Item | Reason |
| --- | --- |
| `sccache` | Redundant with `swatinem/rust-cache` whole-`target` restore. |
| Turborepo for UI | Dropped; npm cache sufficient for a single JS package. Cargo Workspaces handles the Rust side and cannot cache JS tasks. |
| `release: types: [published]` for gates | Too late — artifacts already built. Gate on tag push instead. |
| `cargo mutants` in PR CI | Slow; backlog signal only. Stays in `--deep`/release. |

---

## File Structure

- **Create** `.github/actions/install-tauri-deps/action.yml` — cache apt `.deb` archives (first-party `actions/cache@v4`, user-owned `~/.cache/apt-archives`) + `apt-get install` the 5 Tauri `-dev` libs. Used by `build`/`clippy`/`lint-extras` (PR CI) and `release-verify` (release).
- **Create** `.github/actions/install-cargo-tools/action.yml` — cache the 4 `taiki-e` tool binaries in `~/.cargo/bin`; install on miss only. Used by `lint-extras`.
- **Modify** `.github/workflows/ci.yml` — apt action (3 call sites); dedupe `ui-typecheck` in the `test` job; remove the `miri-changes` + `miri` jobs and their `MIRI_*` env; add a `detect` job + path-filter `if:` on `build`/`fmt`/`ui`; harden the `verify` job `if:`; (optional) `cargo-nextest` + test matrix.
- **Modify** `.github/workflows/release.yml` — add `workflow_dispatch` + `MIRI_*` env; add a Linux `release-verify` job (Miri + deep tests, target/miri cache fix); gate the macOS `release` job on it (`needs: release-verify` + tag-only `if:`).
- **Modify** `scripts/test-fast.sh` — add `--skip-ui-typecheck` flag; (optional) `--nextest` flag.
- **Delete** `scripts/install-ci-deps.sh` — dead after the apt action (only `ci.yml` called it).
- **Delete** `scripts/ci-miri-matrix.sh` — dead after Miri moves to release (only `ci.yml` called it).
- **Modify** `docs/contributing/testing-workflows.md` — Miri is release-only; PR CI scope.
- **Modify** `docs/contributing/releasing.md` — tag → `release-verify` → macOS build → publish.

---

## Task 0: Record baseline CI durations

**Files:** none

- [ ] **Step 1: Capture current job timings**

Run:

```bash
gh run list --workflow=ci.yml --limit=5
```

Pick the most recent **successful** run on `main`, then:

```bash
gh run view <RUN_ID>
```

Record `build`, `clippy`, `lint-extras`, `test`, and `ui` durations. If a recent run included the `miri` job (fires only on `crates/engine/**` or `crates/orchestration/**` changes), record its per-crate duration too. Save as "Baseline" in the PR.

---

## Task 1: Create the `install-tauri-deps` composite action (apt cache)

**Files:**
- Create: `.github/actions/install-tauri-deps/action.yml`

- [ ] **Step 1: Create the composite action**

Create `.github/actions/install-tauri-deps/action.yml` with exactly:

```yaml
name: Install Tauri Linux deps
description: Cache apt .deb archives (first-party actions/cache) and install the Tauri Linux -dev libs. No-op on non-Linux runners.

runs:
  using: composite
  steps:
    - name: Cache apt archives
      if: runner.os == 'Linux'
      uses: actions/cache@v4
      with:
        # User-owned dir so actions/cache can save/restore. Caching /var/cache/apt/archives
        # fails on restore: it is root-owned and tar runs as the runner user. Redirecting apt
        # here also dodges the image's docker-clean hook that rm's the default archives dir.
        path: ~/.cache/apt-archives
        key: apt-tauri-${{ runner.os }}-${{ hashFiles('.github/actions/install-tauri-deps/action.yml') }}
        restore-keys: |
          apt-tauri-${{ runner.os }}-

    - name: Install Tauri Linux deps (cached)
      if: runner.os == 'Linux'
      shell: bash
      run: |
        set -euo pipefail
        # ponytail: caches the deb DOWNLOADS, not installed state — apt still runs dpkg each run.
        # 5 -dev libs not preinstalled on ubuntu-latest; build-essential/curl/wget/file are
        # already on the image, so they are not listed. Upgrade path if version drift causes too
        # many partial misses: also cache /var/lib/apt/lists (root-owned, needs the same chown trick).
        mkdir -p "$HOME/.cache/apt-archives/partial"
        echo "Dir::Cache::archives \"$HOME/.cache/apt-archives\";" | sudo tee /etc/apt/apt.conf.d/99user-archives >/dev/null
        echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' | sudo tee /etc/apt/apt.conf.d/01keep-debs >/dev/null
        sudo apt-get update
        sudo apt-get install -y libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

On a cache hit, `apt-get install` finds the debs in `$HOME/.cache/apt-archives` and skips the network download; `apt-get update` still runs (~5s) so version drift is handled (a changed version re-downloads just that deb).

- [ ] **Step 2: Validate the YAML parses**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/actions/install-tauri-deps/action.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 3: Commit**

```bash
git add .github/actions/install-tauri-deps/action.yml
git commit -m "ci: add install-tauri-deps composite action (cached apt)"
```

---

## Task 2: Wire the apt composite into CI; delete the dead script

**Files:**
- Modify: `.github/workflows/ci.yml` (3 call sites)
- Delete: `scripts/install-ci-deps.sh`

- [ ] **Step 1: Replace all three apt call sites**

The `build` (~55-56), `clippy` (~107-108), and `lint-extras` (~188-189) jobs contain the byte-identical block. In `.github/workflows/ci.yml`, replace **all occurrences** of:

```yaml
      - name: Install Linux system dependencies (Tauri)
        run: ./scripts/install-ci-deps.sh
```

with:

```yaml
      - name: Install Tauri Linux deps (cached)
        uses: ./.github/actions/install-tauri-deps
```

(StrReplace `replace_all: true` — exactly 3 occurrences.)

- [ ] **Step 2: Confirm no remaining references**

```bash
rg -n "install-ci-deps" .
```

Expected: no matches.

- [ ] **Step 3: Delete the dead script**

```bash
git rm scripts/install-ci-deps.sh
```

- [ ] **Step 4: Validate the workflow YAML parses**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml scripts/install-ci-deps.sh
git commit -m "ci: use cached install-tauri-deps; drop install-ci-deps.sh"
```

---

## Task 3: Dedupe `ui-typecheck`

**Why:** The `test` job runs `./scripts/test-fast.sh --execution`, which unconditionally runs `./scripts/verify/ui-typecheck.sh` (`scripts/test-fast.sh:60`). The `ui` job also runs it. ~20–40s wasted plus a duplicate npm setup.

**Files:**
- Modify: `scripts/test-fast.sh`
- Modify: `.github/workflows/ci.yml` (the `test` job's run line, ~line 147)

- [ ] **Step 1: Add `--skip-ui-typecheck` to `test-fast.sh`**

In `scripts/test-fast.sh`, set a flag variable next to `RUN_DESKTOP=0`:

```bash
RUN_EXECUTION=0
RUN_DESKTOP=0
SKIP_UI_TYPECHECK=0
```

Add an arg case in the `while` loop (after the `--desktop` case):

```bash
	--skip-ui-typecheck)
		SKIP_UI_TYPECHECK=1
		shift
		;;
```

Update the `usage` heredoc `Options:` block to include:

```
  --skip-ui-typecheck  Omit the ui typecheck step (run it in a separate CI job instead).
```

Guard the ui-typecheck step (replacing the unconditional `run_step "ui typecheck" ...` line):

```bash
if [[ "$SKIP_UI_TYPECHECK" != "1" ]]; then
	run_step "ui typecheck" "$ROOT/scripts/verify/ui-typecheck.sh"
fi
```

- [ ] **Step 2: Use the flag in the CI `test` job**

In `.github/workflows/ci.yml`, in the `test` job, change:

```yaml
      - name: Run test-fast
        run: ./scripts/test-fast.sh --execution
```

to:

```yaml
      - name: Run test-fast
        run: ./scripts/test-fast.sh --execution --skip-ui-typecheck
```

- [ ] **Step 3: Local parity check — flag works, default unchanged**

```bash
./scripts/test-fast.sh --skip-ui-typecheck --help
./scripts/verify.sh ui-typecheck
```

Expected: `--help` lists `--skip-ui-typecheck`; `ui-typecheck` still passes on its own (the `ui` CI job's path is unchanged).

- [ ] **Step 4: Commit**

```bash
git add scripts/test-fast.sh .github/workflows/ci.yml
git commit -m "ci: skip ui-typecheck in the test job (run once in the ui job)"
```

---

## Task 4: Move Miri + deep checks to `release.yml` (`release-verify`)

**Why:** Miri is ~170–280s per crate on many PRs, not on the critical path but billable. `unsafe_code = "forbid"` + the filtered Miri scope make PR-time Miri optional. Move it (and the heavier `workflow_e2e` + `--desktop` checks) to a `release-verify` job that gates the macOS build on tag push. Also fixes the `target/miri` cache misconfiguration in the move.

**Files:**
- Modify: `.github/workflows/ci.yml` (remove `miri-changes` + `miri` jobs and `MIRI_*` env)
- Modify: `.github/workflows/release.yml` (add `workflow_dispatch`, `MIRI_*` env, `release-verify` job, gate `release`)
- Delete: `scripts/ci-miri-matrix.sh`
- Modify: `docs/contributing/testing-workflows.md`, `docs/contributing/releasing.md`

- [ ] **Step 1: Remove Miri from PR CI**

In `.github/workflows/ci.yml`:
- Delete the `MIRI_NIGHTLY` and `MIRI_TOOLCHAIN` lines from the top-level `env:` block (~lines 14-15). Keep `RUST_CACHE_SHARED_KEY: ci-verify`.
- Delete the entire `miri-changes` job (~lines 242-261) and the entire `miri` job (~lines 263-305).
- Leave the `verify` job's `needs: [build, fmt, clippy, test, ui, lint-extras]` as-is (Miri was never in it).

- [ ] **Step 2: Add `workflow_dispatch` + `MIRI_*` env to `release.yml`**

In `.github/workflows/release.yml`, replace the `on:` block:

```yaml
on:
  push:
    tags:
      - "v*"
```

with:

```yaml
on:
  push:
    tags:
      - "v*"
  workflow_dispatch:
```

Add a top-level `env:` block (above `permissions:`):

```yaml
env:
  MIRI_NIGHTLY: nightly-2026-06-20
  MIRI_TOOLCHAIN: nightly-2026-06-20
```

- [ ] **Step 3: Add the `release-verify` job**

In `.github/workflows/release.yml`, insert this job **before** the `release:` job:

```yaml
  release-verify:
    name: Release verify (Miri + deep tests)
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust (stable)
        uses: ./.github/actions/setup-rust

      - name: Setup Rust (nightly + miri)
        uses: ./.github/actions/setup-rust
        with:
          toolchain: ${{ env.MIRI_NIGHTLY }}
          components: miri

      - name: Install Tauri Linux deps (cached)
        uses: ./.github/actions/install-tauri-deps

      - name: Install cargo-nextest
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest

      - name: Cache Miri sysroot
        uses: actions/cache@v4
        with:
          path: ~/.cache/miri
          key: miri-sysroot-${{ runner.os }}-${{ env.MIRI_NIGHTLY }}-x86_64-unknown-linux-gnu

      - name: Cache Rust + Miri build artifacts
        uses: swatinem/rust-cache@v2
        with:
          # scripts/miri.sh exports CARGO_TARGET_DIR=target/miri for the Miri run only; the deep
          # checks use ./target. swatinem does not read CARGO_TARGET_DIR, so target/miri must be
          # listed explicitly (this also fixes the prior ./target mis-cache from the old ci.yml job).
          workspaces: |
            . -> target
            . -> target/miri
          cache-on-failure: true
          shared-key: release-verify

      - name: Miri (engine + orchestration)
        run: ./scripts/miri.sh

      - name: Deep tests (execution + desktop)
        run: ./scripts/test-fast.sh --execution --desktop --skip-ui-typecheck

      - name: Orchestration E2E
        run: cargo test -p orchestration --test workflow_e2e -- --nocapture
```

Notes: no job-level `CARGO_TARGET_DIR` — `scripts/miri.sh` sets it internally for the Miri run only, so stable deep-check builds go to `./target` and Miri builds go to `target/miri`. `--skip-ui-typecheck` avoids needing Node here (ui-typecheck is the `ui` PR job's responsibility; the macOS build compiles the frontend via vite anyway).

- [ ] **Step 4: Gate the macOS `release` job on `release-verify`**

In `.github/workflows/release.yml`, the `release:` job currently starts:

```yaml
  release:
    strategy:
      fail-fast: false
```

Change to:

```yaml
  release:
    needs: release-verify
    if: startsWith(github.ref, 'refs/tags/v')
    strategy:
      fail-fast: false
```

`if: startsWith(github.ref, 'refs/tags/v')` keeps the macOS build/release tag-only, so `workflow_dispatch` runs `release-verify` alone (for testing the gate without publishing).

- [ ] **Step 5: Delete the dead Miri matrix script**

```bash
git rm scripts/ci-miri-matrix.sh
```

- [ ] **Step 6: Validate both workflow YAMLs parse**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); yaml.safe_load(open('.github/workflows/release.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 7: Update `docs/contributing/testing-workflows.md`**

- In the test-modes table, the Miri row currently ends "…CI runs on Ubuntu". Change that trailing phrase to: "runs in `release.yml` `release-verify` on tag push (Ubuntu); not on PR CI."
- In the `## Miri` section, the paragraph ending "…CI pins `nightly-2026-06-20` for cache stability." → change "CI pins" to "the `release-verify` job pins".
- In the CI paragraph (the one starting "**CI** runs parallel jobs in `.github/workflows/ci.yml`…"), append: "Miri runs at release (tag push) in `release.yml` `release-verify`, not on PR CI."
- Replace the paragraph starting "**CI:** parallel jobs (`build` warm cache → …); separate **`miri`** matrix job runs `./scripts/miri.sh <crate>` per changed Miri-eligible crate…" with: "**CI:** parallel jobs (`build` warm cache → `fmt`, `clippy`, `test`, `ui`, `lint-extras`); PR CI no longer runs Miri. Miri runs in the release workflow's `release-verify` job (tag push or `workflow_dispatch`) on Ubuntu: `./scripts/miri.sh` (both crates), pinning `nightly-2026-06-20` (`MIRI_TOOLCHAIN`) and caching `~/.cache/miri` (sysroot) + `target/miri` (via `rust-cache`)."

- [ ] **Step 8: Update `docs/contributing/releasing.md`**

In the "After merge (maintainer)" numbered list, item 1 currently reads:

> 1. **Release** workflow builds signed macOS artifacts and opens a **draft** GitHub Release.

Replace with:

> 1. **Release** workflow first runs `release-verify` on Ubuntu — Miri over `engine`+`orchestration`, `test-fast --execution --desktop`, and `cargo test -p orchestration --test workflow_e2e`. The macOS build only proceeds if `release-verify` passes, then builds signed artifacts and opens a **draft** GitHub Release.

Also add a line under the "Release train" diagram (after step 5 "Publish…"): "**Tag push → `release-verify` (Linux deep gate) → macOS build → draft Release → publish.**"

- [ ] **Step 9: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/release.yml docs/contributing/testing-workflows.md docs/contributing/releasing.md scripts/ci-miri-matrix.sh
git commit -m "ci: move Miri + deep checks to release-verify; fix target/miri cache"
```

---

## Task 5: Cache the `taiki-e` cargo tools in `lint-extras`

**Why:** `lint-extras` reinstalls `cargo-deny`, `cargo-machete`, `typos`, `cargo-public-api` every run via `taiki-e/install-action` (~20–40s). Cache the binaries; skip the installs on a hit.

**Files:**
- Create: `.github/actions/install-cargo-tools/action.yml`
- Modify: `.github/workflows/ci.yml` (`lint-extras` job, ~lines 191-209)

- [ ] **Step 1: Create the `install-cargo-tools` composite**

Create `.github/actions/install-cargo-tools/action.yml`:

```yaml
name: Install cargo tools
description: Cache the taiki-e cargo tool binaries and install them only on cache miss.

runs:
  using: composite
  steps:
    - name: Cache cargo tool binaries
      id: cargo-tools
      uses: actions/cache@v4
      with:
        # Cache ONLY the 4 tool binaries (not all of ~/.cargo/bin, which overlaps setup-rust
        # and swatinem). ponytail: caches binaries, not versions — bump the key prefix
        # (cargo-tools2-) to refresh when upstream versions move. Upgrade path: pin tool
        # versions in the taiki-e call (tool: cargo-deny@<ver>,...) for a stable key.
        path: |
          ~/.cargo/bin/cargo-deny
          ~/.cargo/bin/cargo-machete
          ~/.cargo/bin/typos
          ~/.cargo/bin/cargo-public-api
        key: cargo-tools-${{ runner.os }}-${{ hashFiles('.github/actions/install-cargo-tools/action.yml') }}
        restore-keys: |
          cargo-tools-${{ runner.os }}-

    - name: Install cargo-deny, cargo-machete, typos, cargo-public-api
      if: steps.cargo-tools.outputs.cache-hit != 'true'
      uses: taiki-e/install-action@v2
      with:
        tool: cargo-deny,cargo-machete,typos,cargo-public-api
```

- [ ] **Step 2: Replace the 4 `taiki-e` steps in `lint-extras`**

In `.github/workflows/ci.yml`, in the `lint-extras` job, replace these four steps (the `Install cargo-deny`, `Install cargo-machete`, `Install typos`, `Install cargo-public-api` `taiki-e/install-action` steps, ~lines 191-209):

```yaml
      - name: Install cargo-deny
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny

      - name: Install cargo-machete
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-machete

      - name: Install typos
        uses: taiki-e/install-action@v2
        with:
          tool: typos

      - name: Install cargo-public-api
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-public-api
```

with the single step:

```yaml
      - name: Install cargo tools (cached)
        uses: ./.github/actions/install-cargo-tools
```

- [ ] **Step 3: Validate YAML parses**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); yaml.safe_load(open('.github/actions/install-cargo-tools/action.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 4: Commit**

```bash
git add .github/actions/install-cargo-tools/action.yml .github/workflows/ci.yml
git commit -m "ci: cache taiki-e cargo tools in lint-extras"
```

---

## Task 6: Path filters — skip Rust/ui jobs on non-Rust PRs

**Why:** Docs/UI-only PRs pay the full `build` (~4–5 min) today. Detect changed areas and skip irrelevant jobs.

**Files:**
- Modify: `.github/workflows/ci.yml` (new `detect` job; `if:` on `build`/`fmt`/`ui`; harden `verify`)

- [ ] **Step 1: Add a `detect` job**

In `.github/workflows/ci.yml`, add this job right after the `version:` job (before `build:`):

```yaml
  detect:
    name: Detect changed areas
    runs-on: ubuntu-latest
    outputs:
      rust: ${{ steps.f.outputs.rust }}
      ui: ${{ steps.f.outputs.ui }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dorny/paths-filter@v3
        id: f
        with:
          # rust excludes crates/ui/** so UI-only PRs don't trigger the Rust chain.
          filters: |
            rust:
              - 'crates/engine/**'
              - 'crates/providers/**'
              - 'crates/orchestration/**'
              - 'crates/desktop/**'
              - 'crates/workspace-checks/**'
              - 'Cargo.lock'
              - 'Cargo.toml'
              - 'rust-toolchain.toml'
              - '.github/workflows/ci.yml'
              - '.github/actions/**'
              - 'scripts/**'
            ui:
              - 'crates/ui/**'
```

- [ ] **Step 2: Gate `build` and `fmt` on `rust`; `ui` on `ui`**

For the `build` job, add `needs` + `if:` to its header (it currently has neither):

```yaml
  build:
    name: Build (warm cache)
    needs: detect
    if: needs.detect.outputs.rust == 'true'
    runs-on: ubuntu-latest
```

For the `fmt` job:

```yaml
  fmt:
    name: fmt
    needs: detect
    if: needs.detect.outputs.rust == 'true'
    runs-on: ubuntu-latest
```

For the `ui` job:

```yaml
  ui:
    name: ui
    needs: detect
    if: needs.detect.outputs.ui == 'true'
    runs-on: ubuntu-latest
```

`clippy`, `test`, and `lint-extras` keep `needs: build` (they cascade-skip automatically when `build` is skipped).

- [ ] **Step 3: Harden the `verify` job so skipped deps still yield a passing gate**

Replace the `verify` job:

```yaml
  verify:
    name: Verify (blocking)
    needs: [build, fmt, clippy, test, ui, lint-extras]
    runs-on: ubuntu-latest
    steps:
      - run: echo "All verify checks passed"
```

with:

```yaml
  verify:
    name: Verify (blocking)
    needs: [detect, build, fmt, clippy, test, ui, lint-extras]
    if: ${{ !cancelled() }}
    runs-on: ubuntu-latest
    steps:
      - name: Fail if any required job failed
        if: ${{ contains(join(needs.*.result, ','), 'failure') }}
        run: exit 1
      - run: echo "All verify checks passed"
```

`!cancelled()` lets `verify` run even when deps were skipped (docs-only PR); the fail-step makes it fail only if a dep actually failed. If `verify` is a required branch-protection check, this keeps it passing on skip-only runs.

- [ ] **Step 4: Validate YAML parses**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: path-filter Rust/ui jobs off on non-Rust PRs"
```

---

## Task 7 (OPTIONAL — only if `test` is still a bottleneck after 1–6): `cargo-nextest` in the PR `test` job

**Gate:** Measure the `test` job after Tasks 1–6. Only do this if it's still a bottleneck. `cargo-nextest` runs test binaries in parallel; the repo already uses it for Miri.

**Files:**
- Modify: `scripts/test-fast.sh` (add `--nextest`)
- Modify: `scripts/verify/test-engine.sh`, `test-providers.sh`, `test-orchestration-lib.sh`, `test-workspace-checks.sh` (honor `USE_NEXTEST`)
- Modify: `.github/workflows/ci.yml` (`test` job installs nextest + passes `--nextest`)

- [ ] **Step 1: Make the `verify/test-*.sh` scripts honor `USE_NEXTEST`**

In each of `scripts/verify/test-engine.sh`, `test-providers.sh`, `test-orchestration-lib.sh`, `test-workspace-checks.sh`, replace the `exec cargo test …` line with a conditional that uses `cargo nextest run` when `USE_NEXTEST=1`. Example for `test-engine.sh`:

```bash
if [[ "${USE_NEXTEST:-0}" == "1" ]]; then
	exec cargo nextest run -p engine --lib "$@"
else
	exec cargo test -p engine --lib "$@"
fi
```

Mirror the same pattern in the other three, keeping each script's existing `-p <crate>` and target flags (`--lib` etc.) identical — only swap `cargo test` → `cargo nextest run` in the `USE_NEXTEST` branch.

- [ ] **Step 2: Add `--nextest` to `test-fast.sh`**

Add `USE_NEXTEST=0` next to `SKIP_UI_TYPECHECK=0`, an arg case `--nextest) USE_NEXTEST=1; shift ;;`, and export it before the `run_step` calls:

```bash
export USE_NEXTEST
```

Document `--nextest` in the `usage` heredoc.

- [ ] **Step 3: Wire nextest into the CI `test` job**

In `.github/workflows/ci.yml`, in the `test` job, add an install step (after `Setup Rust`) and update the run line:

```yaml
      - name: Install cargo-nextest
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest
```

and change the run line to:

```yaml
      - name: Run test-fast
        run: ./scripts/test-fast.sh --execution --skip-ui-typecheck --nextest
```

- [ ] **Step 4: Local parity + measure**

```bash
USE_NEXTEST=1 ./scripts/verify/test-engine.sh
./scripts/test-fast.sh --execution --skip-ui-typecheck --nextest
```

Expected: both pass. Compare the CI `test` job duration before/after; keep only if it drops meaningfully.

- [ ] **Step 5: Commit**

```bash
git add scripts/test-fast.sh scripts/verify/test-engine.sh scripts/verify/test-providers.sh scripts/verify/test-orchestration-lib.sh scripts/verify/test-workspace-checks.sh .github/workflows/ci.yml
git commit -m "ci: run PR tests through cargo-nextest"
```

---

## Task 8 (OPTIONAL — only if `test` is still the critical path after 1–7): Parallel test matrix

**Gate:** Only if the `test` job remains the slowest single job. Splits it so the critical path is the slowest leg, not the sum.

**Files:**
- Modify: `.github/workflows/ci.yml` (replace the `test` job with a matrix; update `verify` needs)

- [ ] **Step 1: Replace the `test` job with a matrix**

Replace the `test` job with:

```yaml
  test:
    name: test (${{ matrix.leg }})
    needs: [detect, build]
    if: needs.detect.outputs.rust == 'true'
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        leg: [engine, providers, orchestration-lib, execution]
        include:
          - leg: engine
            script: ./scripts/verify/test-engine.sh
          - leg: providers
            script: ./scripts/verify/test-providers.sh
          - leg: orchestration-lib
            script: ./scripts/verify/test-orchestration-lib.sh
          - leg: execution
            script: ./scripts/verify/test-execution.sh
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: ./.github/actions/setup-rust

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: "22"
          cache: npm
          cache-dependency-path: crates/ui/package-lock.json

      - name: Install npm dependencies
        run: npm ci --prefix crates/ui

      - name: Cache Rust build artifacts
        uses: swatinem/rust-cache@v2
        with:
          shared-key: ${{ env.RUST_CACHE_SHARED_KEY }}
          cache-on-failure: true

      - name: Run test leg
        run: ${{ matrix.script }}
```

(`test-execution.sh` runs `cargo test -p orchestration --test workflow_acceptance`; the `execution` leg replaces the old `--execution` lane. `ui-typecheck` is not in the matrix — it stays in the `ui` job.)

- [ ] **Step 2: Update `verify` to wait for all matrix legs**

Change the `verify` job's `needs:` to include `test` (the matrix surfaces as a single `test` dependency):

```yaml
    needs: [detect, build, fmt, clippy, test, ui, lint-extras]
```

(`test` here refers to the whole matrix; `verify` waits for every leg.)

- [ ] **Step 3: Validate YAML parses**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
```

Expected: `ok`

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: split test into parallel matrix legs"
```

---

## Task 9: End-to-end validation

**Files:** none

- [ ] **Step 1: Push and open a PR**

```bash
git push -u origin HEAD
gh pr create --title "ci: cut PR CI time — apt cache, dedupe ui-typecheck, move Miri to release" --body "$(cat <<'EOF'
## Summary
- Cache Tauri apt deps via first-party `actions/cache@v4` (build/clippy/lint-extras).
- Dedupe `ui-typecheck` (test job skips it; ui job owns it).
- Move Miri + deep checks (`workflow_e2e`, `--desktop`) to `release.yml` `release-verify`; gate macOS build on it (tag push, not release:published). Fix `target/miri` cache.
- Cache `taiki-e` cargo tools in lint-extras.
- Path-filter Rust/ui jobs off on non-Rust PRs.

## Baseline durations
(recorded in Task 0 — fill in)

## Test plan
- [ ] PR run: build/clippy/lint-extras apt cache-hit on 2nd run; `ui-typecheck` runs only in `ui`; no `miri` jobs.
- [ ] Docs-only PR: only `detect` + `version` + `verify` run (~1 min).
- [ ] `workflow_dispatch` on release.yml: `release-verify` runs (Miri + deep tests) without publishing.
- [ ] Tag push: `release-verify` gates the macOS build.
EOF
)"
```

- [ ] **Step 2: Confirm the PR run is green; check the 2nd run for cache hits**

```bash
gh pr checks --watch
git commit --allow-empty -m "ci: re-trigger for cache-hit verification" && git push
gh run view <SECOND_RUN_ID> --log | rg -i "cache-hit|Cache restored"
```

Expected: apt step `cache-hit=true`; cargo-tools step `cache-hit=true` (2nd run); `miri` jobs absent.

- [ ] **Step 3: Compare durations to baseline**

```bash
gh run view <SECOND_RUN_ID>
```

Expected: `build`/`clippy`/`lint-extras`/`test` durations lower than baseline (apt + tool-cache + dedupe). Critical path ~7–8 min.

- [ ] **Step 4: Validate `release-verify` via `workflow_dispatch`**

```bash
gh workflow run release.yml
gh run list --workflow=release.yml --limit=1
gh run watch <RUN_ID>
```

Expected: `release-verify` runs on Ubuntu, Miri + `test-fast --execution --desktop --skip-ui-typecheck` + `workflow_e2e` pass; the macOS `release` job does **not** run (no tag). Do not rely on a real tag until this passes.

- [ ] **Step 5: Merge once green and measured**

Merge per repo convention. Local parity: `./scripts/verify.sh` remains the full local gate; PR CI is now a subset, release is the deep gate.

---

## Notes

- **Why first-party `actions/cache@v4` for apt, not `awalsh128/cache-apt-pkgs-action`:** `awalsh128` is used (~350 stars, `v1.6.1` June 2026) but has an open "looking for co-maintainers" notice, 28 open issues, and a forced Node 20→24 transitive bump. `actions/cache` is already a repo dependency. Tradeoff: caches deb **downloads**, not installed state; `apt-get update` always runs to handle drift. Win is modest (~15–30s × 3 jobs).
- **Miri move is the highest-value PR-CI change:** removes ~170–280s/crate of billable work from engine/orchestration PRs; the `target/miri` cache fix is folded into the new `release-verify` job. The macOS build is gated on `release-verify` at tag push (`if: startsWith(github.ref, 'refs/tags/v')`), so `workflow_dispatch` can test the gate without publishing. **Do not** gate on `release: types: [published]` — too late.
- **Why not sccache / Turborepo / mutants in PR CI:** see the Tier 4 table.
- **Path-filter + required checks:** `verify` uses `if: !cancelled()` + a fail-on-any-failure step so docs-only PRs (all deps skipped) still produce a passing `verify`. If `verify` is a required branch-protection check, this avoids "skipped ≠ passed" blocking.
- **`dorny/paths-filter@v3`** is a third-party action (well-maintained, unlike `awalsh128`'s bus-factor). For a no-new-dep alternative, reuse the repo's existing shell-detect pattern (`ci-miri-matrix.sh` style) — more code, no third-party action. Pin `dorny/paths-filter` to a SHA for supply-chain hardening if adopted permanently.
- **Tasks 7–8 are gated on measurement**, per the suggested order ("nextest + test matrix — if still needed after 1–5"). Don't adopt them unless a before/after shows a real bottleneck remains.
