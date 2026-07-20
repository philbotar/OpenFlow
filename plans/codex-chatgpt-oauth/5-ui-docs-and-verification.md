# Slice 5: Settings UI, Documentation, and Verification

## Goal

- Deliver the sign-in/device/connected/disconnect UI, document private-backend constraints, and prove the feature across focused and canonical verification lanes.

## Current Question

- Question: None.
- Recommended answer: Poll the tagged login status from the provider settings panel and refresh provider readiness after connection or disconnect.
- Reason: This keeps OAuth state local to the credentials UI and avoids expanding the global context with secrets or protocol details.

## Codebase Findings

- UI provider behavior is split across `constants/providers.ts`, `useSettings.ts`, typed `api.ts`, `ProvidersSection.tsx`, and global styles/tests.
- Codex must bypass API-key load/save/delete behavior and appear directly after OpenAI in display order.
- Desktop E2E IPC mocks and provider settings tests must learn the new commands to keep the app suite deterministic.
- The feature cannot prove real ChatGPT entitlement or private-backend originator compatibility without an interactive live-account smoke.
- Test command: `npm --prefix crates/ui run test -- src/settings/ProvidersSection.test.tsx src/api.test.ts`

## Ownership

- Modify: `crates/ui/src/lib/types/index.ts` for the tagged login status DTO.
- Modify: `crates/ui/src/api.ts` and its tests for start/status/cancel/disconnect wrappers.
- Modify: `crates/ui/src/constants/providers.ts` for provider order.
- Modify: `crates/ui/src/context/appProvider/useSettings.ts` to bypass API-key actions and refresh readiness.
- Modify: `crates/ui/src/settings/ProvidersSection.tsx` for sign-in, pending browser/device, connected, error, cancel, retry, and disconnect states.
- Modify: `crates/ui/src/styles/index.css` for focused status/code presentation.
- Modify: UI app tests and desktop E2E IPC mocks/settings-provider tests.
- Modify: `docs/architecture/provider-adapters.md`, `docs/getting-started/README.md`, `docs/reference/README.md`, and `docs/troubleshooting/README.md` where their existing provider/setup tables require Codex coverage.

## Contract Detail

- The Codex panel never renders API-key or AWS controls.
- Starting login immediately shows browser-waiting feedback; device fallback presents a selectable code and verification link; cancel is available only while pending.
- Connected state shows safe account email when available, a ready indicator, and Disconnect with confirmation behavior matching existing settings conventions.
- Failed/cancelled state offers retry and preserves an already connected session unless disconnect was explicitly chosen.
- Documentation labels this as ChatGPT subscription authentication through an unofficial/private backend integration that may change, states plaintext local storage, and distinguishes it from OpenAI API-key billing.
- Verification reports the live-account smoke as manual unless the user supplies an account interaction during this run.

## Steps

- [ ] **Step 1: Write failing API/component tests**
  - Prove command names/serialization and every panel state, including device code, cancel, connected email, retry, disconnect, provider order, and absence of secret fields.
- [ ] **Step 2: Verify RED**
  - Run: `npm --prefix crates/ui run test -- src/settings/ProvidersSection.test.tsx src/api.test.ts`
  - Expected: FAIL because Codex IPC wrappers and UI do not exist.
- [ ] **Step 3: Implement the minimal UI**
  - Add typed wrappers, polling lifecycle/cleanup, readiness refresh, panel states, and focused styles.
  - Extend app/E2E mocks without changing other provider behavior.
- [ ] **Step 4: Verify UI GREEN**
  - Run: `npm --prefix crates/ui run test -- src/settings/ProvidersSection.test.tsx src/api.test.ts`
  - Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx`
  - Run: `npm --prefix crates/ui run typecheck`
  - Expected: PASS with no secrets in mocked IPC payloads.
- [ ] **Step 5: Update documentation from verified behavior**
  - Document setup, storage, readiness, refresh/disconnect, troubleshooting, and private-backend fragility.
  - Keep model names and command names aligned with the final implementation.
- [ ] **Step 6: Run cross-crate and canonical gates**
  - Run: `cargo test -p providers`
  - Run: `cargo test -p orchestration --lib`
  - Run: `cargo test -p desktop`
  - Run: `./scripts/check-architecture.sh`
  - Run: `./scripts/verify.sh test clippy arch ui-typecheck ui-test`
  - Expected: PASS. Record any unrelated pre-existing failure with its exact repro and evidence.
- [ ] **Step 7: Record the live-smoke boundary**
  - If an interactive ChatGPT sign-in is available, run one streaming text/tool workflow and one forced-refresh case.
  - Otherwise record the live smoke as unperformed rather than implying account entitlement was verified.

## Maintainability Gate

- [ ] Login polling is cleaned up on provider change/unmount.
- [ ] No credential value crosses IPC or appears in UI tests.
- [ ] UI behavior is driven by the tagged backend state rather than inferred strings.
- [ ] Documentation matches tested behavior and names the private integration risk.
- [ ] Final verification distinguishes fixture coverage from live-account proof.

## Result

- Status: Pending
- Verification: Not run.

