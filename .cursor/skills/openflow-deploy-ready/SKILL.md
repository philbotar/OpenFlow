---
name: openflow-deploy-ready
description: >-
  Set-and-forget OpenFlow desktop release. Invoking this skill authorizes the
  full path: version/tag checks, local gates, wait PR CI, cut desktop bump if
  needed, merge, tag, wait Release workflow, stop at draft. Use when user
  invokes openflow-deploy-ready or asks to release / ship / deploy.
disable-model-invocation: true
---

# openflow-deploy-ready

**Deploy = new GitHub Release** (tag `vX.Y.Z`), not merge to `main`.

Source of truth: `docs/contributing/releasing.md`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`.

## Set-and-forget contract

**Invoking this skill = authorize the full deploy path.** No extra phrases. No check-only mode.

User invokes once. Agent **owns the loop** through: version/tag correctness → local gates → wait PR CI → cut desktop bump if needed → merge to `main` when green → tag + push → wait Release workflow → stop at **draft** GitHub Release (human publishes after asset review).

Do **not** stop after a single snapshot. Do **not** ask permission for checks, waits, cuts, commits, pushes, or tagging.

| Do | Don't |
| --- | --- |
| Wait for PR CI / Release workflow to finish | Ask user to "check CI and come back" |
| Re-run failed gates after fixing | Dump a checklist and exit mid-flight |
| Poll with backoff; report only on state change or done | Spam progress every few seconds |
| Cut, commit, push, tag when GO | Ask "should I tag?" after GO |
| Interrupt only for blockers below | Ask what invoke meant |

**Interrupt user only when:**

1. PR base ≠ `main` (or no PR and not on main) — cannot merge path
2. Tag/version collision — need new version choice
3. CI failure needs a product/architecture decision (not a clear fix)
4. Dirty tree / conflicting local work that would clobber unrelated changes

**Do not** auto-publish the draft Release (updater goes live on publish). Report draft URL + assets; user publishes when ready.

## Mode

| Starting state | Action |
| --- | --- |
| Desktop == `origin/main` | Run `cut-release.sh`, commit, continue ship |
| Desktop already **>** `origin/main` | Use that version as planned tag |
| On `main` | Cut if needed, then tag |

```bash
git fetch origin main --tags
```

## Phases (run in order; do not skip waits)

Track with todos. Each phase must reach PASS or terminal NO-GO before next.

```
- [ ] A. Intake (branch → main, dirty, mode)
- [ ] B. Versions / tags (highest worry)
- [ ] C. Local gates (verify + release-verify E2E)
- [ ] D. Wait PR CI (set-and-forget)
- [ ] E. Verdict
- [ ] F. Ship (tag + push) + wait Release → draft
```

---

### A. Intake

```bash
rtk git status
rtk git branch --show-current
rtk git diff --stat
gh pr view --json baseRefName,headRefName,state,url,title,statusCheckRollup 2>/dev/null || true
```

Allowed: on `main`, or PR **base = `main`**, or user confirms they will open/retarget into `main`.

**NO-GO** if base ≠ `main` and user won't retarget.

---

### B. Versions / tags (highest priority)

```bash
git fetch origin main --tags
BASE="$(git merge-base HEAD origin/main)"
VERSION_CHECK_BASE="$BASE" ./scripts/check-version-bump.sh
VERSION_CHECK_BASE="$BASE" ./scripts/check-release-version.sh

VER=$(jq -r '.version' crates/desktop/tauri.conf.json)
MAIN_VER=$(git show origin/main:crates/desktop/tauri.conf.json | jq -r .version)
git tag -l "v${VER}"
git ls-remote --tags origin "refs/tags/v${VER}"
gh release view "v${VER}" --json isDraft,url,isPrerelease 2>/dev/null || true
git tag -l 'v*' | sort -V | tail -5
```

Canonical trio must match: `tauri.conf.json`, `crates/desktop/Cargo.toml`, `crates/desktop/package.json`.

If desktop == `origin/main`: cut first, then re-read `VER`.
If desktop already > main: that is the planned tag (`v${VER}`).

```bash
./scripts/cut-release.sh --dry-run
./scripts/cut-release.sh
# commit + push on release branch or main
```

**NO-GO** if `v${VER}` already on origin — never retag; bump again.

Auto-fix clear version sync/bump failures; re-run scripts until PASS.

---

### C. Local gates

Mirror CI + what `release.yml` `release-verify` runs:

```bash
./scripts/verify.sh
./scripts/miri.sh
./scripts/test-fast.sh --execution --desktop --skip-ui-typecheck
cargo test -p orchestration --test workflow_e2e -- --nocapture
```

Run in parallel where safe (`verify` vs miri/e2e may share target — prefer sequential if contention). Fix failures; re-run until green or unblockable NO-GO.

Signing (optional; do if first release / updater suspect):

```bash
./scripts/validate-tauri-signing.sh
```

---

### D. Wait PR CI (set-and-forget)

If a PR exists for this branch:

1. Ensure latest commit is pushed (`git push -u origin HEAD` if needed).
2. **Block until checks complete** — do not return early:

```bash
# Preferred: watch until terminal
gh pr checks --watch

# If --watch unavailable / flaky, poll:
# every 60–120s: gh pr checks
# exit loop when no pending/in_progress (or equivalent)
```

3. On failure: diagnose logs (`gh run view --log-failed`), fix in scope, push, **re-enter wait**. Loop until green or unblockable.
4. Behind `main` with unrelated CI fail: merge/rebase `origin/main`, push, re-wait.

If **on main** with no PR (post-merge cut): skip PR wait; local gates + version checks are the pre-tag gate. CI on `push` to main is informational — still:

```bash
gh run list --branch main --limit 5
# if a run is in progress for this SHA, wait:
gh run watch
```

**Do not** ask the user to refresh GitHub. Agent watches.

---

### E. Verdict

Only emit when phases A–D are terminal (or unblockable NO-GO):

```markdown
## Deploy readiness: GO | NO-GO

**Branch:** <name> → main
**Desktop:** <main_ver> → <branch_ver>
**Planned tag:** vX.Y.Z
**Tag free on origin:** yes | no
**Versions:** crate bumps PASS/FAIL · desktop sync PASS/FAIL
**Gates:** local verify PASS/FAIL · release-verify E2E PASS/FAIL · PR CI PASS/FAIL/n/a

### Blockers
- …

### Next
Proceeding to tag + Release watch (phase F).
```

On GO, continue to F without asking.

---

### F. Ship + wait Release

After merge to `main` and desktop version on `main` == planned tag:

```bash
git checkout main && git pull origin main
jq -r '.version' crates/desktop/tauri.conf.json   # must match tag
git tag "v${VER}"
git push origin "v${VER}"
```

Then **wait for Release workflow** (do not stop at tag push):

```bash
gh run list --workflow=release.yml --limit 5
gh run watch <run-id>    # or poll until release-verify + macOS jobs done
gh release view "v${VER}" --json isDraft,url,assets
```

Stop at **draft**. Report asset list + URL. Publish only if user later says publish.

If `release-verify` fails: pull logs, fix on a follow-up commit, **new patch bump + new tag** (never reuse failed tag if artifacts half-published — follow releasing.md).

## Wait / poll rules

- Prefer `gh pr checks --watch` / `gh run watch` over sleep loops.
- If polling: 60–120s backoff; use `AwaitShell` / background + notify; one-line status only on change (pending → fail/pass).
- Max patience: keep watching through normal CI length (verify ~tens of minutes; Release longer). Do not give up after one poll.
- Session end mid-wait: leave clear "still waiting on \<run\>" + exact resume command; prefer not ending until watch returns.

## Hard rules

- Merge ≠ release.
- Tag `v*` must match `tauri.conf.json` or Release job fails.
- Do not retag a shipped version.
- Normal feature PRs (skill not invoked): no desktop bump.
- This skill invoked: cut desktop bump if needed, then ship.
- Set-and-forget = agent waits; user is not the CI watcher.
- Tag + push on GO; do not publish the draft unless user says publish.
