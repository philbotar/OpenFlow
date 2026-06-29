# Bedrock Provider Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Amazon Bedrock work with legacy OpenFlow settings and make Bedrock failures explain themselves in Settings.

**Architecture:** Keep AWS transport and Bedrock SDK calls inside `crates/providers`; keep settings resolution, migration, and IPC-facing diagnostics inside `crates/orchestration`; keep Tauri commands as thin adapters in `crates/desktop`; keep presentation in `crates/ui`. The first slice fixes the current work-machine config path: old settings stored the AWS profile in `providers.bedrock.api_key`, but current code ignores Bedrock `api_key` and reads only `aws_profile`.

**Tech Stack:** Rust 2024, `aws-config`, `aws-sdk-bedrock`, `aws-sdk-bedrockruntime`, Tauri commands, SolidJS/TypeScript, Vitest.

---

## Current Evidence

- Local AWS CLI profile list contains one profile: `openflow-bedrock`.
- Local OpenFlow settings at `/Users/philipbotar/Library/Application Support/openflow/settings.json` have:

```json
{
  "providers": {
    "bedrock": {
      "base_url": "ap-southeast-2",
      "default_model": "amazon.nova-pro-v1:0",
      "api_key": "openflow-bedrock",
      "aws_profile": ""
    }
  }
}
```

- Current code ignores `api_key` for Bedrock in `crates/orchestration/src/settings/provider.rs`, while `ProviderProfile::normalized()` clears Bedrock `api_key` in `crates/orchestration/src/settings/model.rs`.
- Network-approved probes failed with `Error when retrieving token from sso: Token has expired and refresh failed`, so the user must also run:

```bash
aws sso login --profile openflow-bedrock
```

That command is not enough by itself unless OpenFlow also uses the `openflow-bedrock` profile through Settings or `AWS_PROFILE`.

## File Structure

- Modify `crates/orchestration/src/settings/model.rs`
  - Migrate legacy Bedrock profile values from `api_key` into `aws_profile` during settings normalization.
  - Preserve the existing rule that Bedrock must not store API keys.

- Modify `crates/orchestration/src/settings/provider.rs`
  - Include `AWS_REGION` in `ProviderEnv::from_system()`.
  - Keep `AWS_PROFILE` resolution order: settings `aws_profile` first, `ProviderEnv` second, live process env last.
  - Add tests for region env fallback and profile env fallback.

- Modify `crates/orchestration/src/api.rs`
  - Add a serializable Bedrock diagnostic DTO with resolved profile, region, default model, available model status, and message.

- Modify `crates/orchestration/src/settings/facade.rs`
  - Add `check_bedrock_connection(settings)` that merges persisted settings, resolves the Bedrock profile, calls model discovery, and returns actionable status.

- Modify `crates/orchestration/src/backend/mod.rs`
  - Delegate `check_bedrock_connection()` to `SettingsFacade`.

- Modify `crates/desktop/src/lib.rs`
  - Add a thin Tauri command `check_bedrock_connection`.

- Modify `crates/ui/src/lib/types/index.ts`
  - Mirror the new diagnostic DTO.

- Modify `crates/ui/src/api.ts`
  - Add `checkBedrockConnection(settings)`.

- Modify `crates/ui/src/port.ts`
  - Add the new command to `UiDesktopOutboundPort`.

- Modify `crates/ui/src/context/AppContext.tsx` and `crates/ui/src/context/AppProvider.tsx`
  - Expose Bedrock diagnostic state and handler.

- Modify `crates/ui/src/settings/ProvidersSection.tsx`
  - Add a Bedrock-only diagnostic button and compact result rows.

- Modify tests:
  - `crates/orchestration/src/settings/model.rs`
  - `crates/orchestration/src/settings/provider.rs`
  - `crates/orchestration/src/backend/tests.rs`
  - `crates/ui/src/settings/ProvidersSection.test.tsx`

---

### Task 1: Migrate Legacy Bedrock Profile Stored In `api_key`

**Files:**
- Modify: `crates/orchestration/src/settings/model.rs`
- Test: `crates/orchestration/src/settings/model.rs`

- [ ] **Step 1: Write the failing migration test**

Add this test inside `#[cfg(test)] mod tests` in `crates/orchestration/src/settings/model.rs`:

```rust
#[test]
fn normalized_migrates_legacy_bedrock_api_key_to_aws_profile() {
    let mut settings = AppSettings::default();
    let profile = settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile");
    profile.api_key = " openflow-bedrock ".to_string();
    profile.aws_profile.clear();

    let normalized = settings.normalized();
    let profile = normalized
        .providers
        .get(&ProviderId::from("bedrock"))
        .expect("bedrock profile");

    assert_eq!(profile.aws_profile, "openflow-bedrock");
    assert!(profile.api_key.is_empty());
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```bash
cargo test -p orchestration settings::model::tests::normalized_migrates_legacy_bedrock_api_key_to_aws_profile -- --nocapture
```

Expected: FAIL because `aws_profile` remains empty after `normalized()`.

- [ ] **Step 3: Add the migration helper**

In `crates/orchestration/src/settings/model.rs`, add this helper near `impl AppSettings`:

```rust
fn migrate_bedrock_legacy_profile(profile: &mut ProviderProfile) {
    let legacy_profile = profile.api_key.trim();
    if profile.aws_profile.trim().is_empty() && !legacy_profile.is_empty() {
        profile.aws_profile = legacy_profile.to_string();
    }
    profile.api_key.clear();
}
```

Then replace the current Bedrock cleanup block in `AppSettings::normalized()`:

```rust
if let Some(profile) = self.providers.get_mut(&ProviderId::from("bedrock")) {
    profile.api_key.clear();
}
```

with:

```rust
if let Some(profile) = self.providers.get_mut(&ProviderId::from("bedrock")) {
    migrate_bedrock_legacy_profile(profile);
}
```

- [ ] **Step 4: Verify the migration tests pass**

Run:

```bash
cargo test -p orchestration settings::model::tests::normalized_migrates_legacy_bedrock_api_key_to_aws_profile settings::model::tests::normalized_clears_bedrock_api_key -- --nocapture
```

Expected: PASS for both tests.

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/settings/model.rs
git commit -m "fix: migrate legacy Bedrock AWS profile setting"
```

---

### Task 2: Resolve Bedrock Region From Environment Correctly

**Files:**
- Modify: `crates/orchestration/src/settings/provider.rs`
- Test: `crates/orchestration/src/settings/provider.rs`

- [ ] **Step 1: Write the failing `AWS_REGION` fallback test**

Add this test inside `#[cfg(test)] mod tests` in `crates/orchestration/src/settings/provider.rs`:

```rust
#[test]
fn bedrock_region_falls_back_to_aws_region_env() {
    let mut settings = AppSettings {
        active_provider: ProviderId::from("bedrock"),
        ..Default::default()
    };
    settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile")
        .base_url
        .clear();

    let resolved = resolve_provider_config(
        &settings,
        None,
        &ProviderEnv::from_pairs([("AWS_REGION", "ap-southeast-2")]),
    )
    .unwrap();

    let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
        panic!("expected Bedrock adapter");
    };
    assert_eq!(config.region, "ap-southeast-2");
    assert!(matches!(
        resolved.auth,
        AuthConfig::AwsCredentials {
            region: ref resolved_region,
            ..
        } if resolved_region == "ap-southeast-2"
    ));
}
```

- [ ] **Step 2: Run the test and confirm current behavior**

Run:

```bash
cargo test -p orchestration settings::provider::tests::bedrock_region_falls_back_to_aws_region_env -- --nocapture
```

Expected: PASS for direct `ProviderEnv::from_pairs`; this proves resolver behavior already supports `AWS_REGION` when present.

- [ ] **Step 3: Write a pure unit test for `ProviderEnv::from_system` env-name collection**

Add a private helper above `impl ProviderEnv`:

```rust
fn provider_env_var_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for spec in providers::builtin_provider_specs() {
        if let Some(env_var) = spec.auth.env_var() {
            names.push(env_var);
        }
        if let AuthSpec::AwsCredentials { region_env_var, .. } = spec.auth {
            names.push(region_env_var);
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}
```

Add this test:

```rust
#[test]
fn provider_env_var_names_include_bedrock_profile_and_region() {
    let names = provider_env_var_names();

    assert!(names.contains(&"AWS_PROFILE"));
    assert!(names.contains(&"AWS_REGION"));
}
```

- [ ] **Step 4: Update `ProviderEnv::from_system()` to use the helper**

Replace the current `from_system()` body:

```rust
let values = providers::builtin_provider_specs()
    .iter()
    .filter_map(|spec| spec.auth.env_var())
    .filter_map(|env_var| {
        std::env::var(env_var)
            .ok()
            .map(|value| (env_var.to_string(), value))
    })
    .collect();
Self { values }
```

with:

```rust
let values = provider_env_var_names()
    .into_iter()
    .filter_map(|env_var| {
        std::env::var(env_var)
            .ok()
            .map(|value| (env_var.to_string(), value))
    })
    .collect();
Self { values }
```

- [ ] **Step 5: Verify provider settings tests**

Run:

```bash
cargo test -p orchestration settings::provider::tests::bedrock_region_falls_back_to_aws_region_env settings::provider::tests::provider_env_var_names_include_bedrock_profile_and_region -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/settings/provider.rs
git commit -m "fix: include Bedrock region in provider env resolution"
```

---

### Task 3: Add A Bedrock Connection Diagnostic DTO And Backend Method

**Files:**
- Modify: `crates/orchestration/src/api.rs`
- Modify: `crates/orchestration/src/settings/facade.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Test: `crates/orchestration/src/backend/tests.rs`

- [ ] **Step 1: Add the diagnostic DTO**

In `crates/orchestration/src/api.rs`, add this struct after `ProviderReadiness`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockConnectionDiagnostic {
    pub ok: bool,
    pub profile: Option<String>,
    pub region: String,
    pub default_model: Option<String>,
    pub default_model_available: bool,
    pub discovered_model_count: usize,
    pub message: String,
}
```

- [ ] **Step 2: Add a failing backend unit test for migrated profile diagnostics without network**

Add this test to `crates/orchestration/src/backend/tests.rs`:

```rust
#[cfg_attr(miri, ignore)]
#[test]
fn bedrock_readiness_uses_migrated_legacy_profile() {
    let (backend, dir) = backend();
    let store = FileSettingsStore::new(dir.path().join("settings.json"));
    let mut settings = store.load().unwrap();
    settings.active_provider = ProviderId::from("bedrock");
    let profile = settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile");
    profile.base_url = "ap-southeast-2".to_string();
    profile.api_key = "openflow-bedrock".to_string();
    profile.aws_profile.clear();
    store.save_raw(&settings).unwrap();

    let loaded = store.load().unwrap();
    let readiness = backend.resolve_provider_readiness(&loaded, None);

    assert!(readiness.ready);
    assert_eq!(readiness.provider, "Amazon Bedrock");
}
```

- [ ] **Step 3: Add `check_bedrock_connection()` in `SettingsFacade`**

Update the import in `crates/orchestration/src/settings/facade.rs`:

```rust
use crate::api::{BedrockConnectionDiagnostic, ProviderReadiness, WorkflowValidationSummary};
```

Add this method inside `impl SettingsFacade` after `refresh_bedrock_models()`:

```rust
/// # Errors
/// Returns an error if settings cannot be loaded.
pub async fn check_bedrock_connection(
    &self,
    settings: &AppSettings,
) -> Result<BedrockConnectionDiagnostic, BackendError> {
    let mut merged = settings.clone();
    merge_preserved_api_keys(&mut merged, &self.store.load()?);
    let profile = merged
        .providers
        .get(&ProviderId::from("bedrock"))
        .ok_or_else(|| {
            BackendError::from(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "bedrock provider profile not found",
            ))
        })?;
    let region = profile.base_url.trim().to_string();
    let profile_name = profile
        .aws_profile
        .trim()
        .is_empty()
        .then_some(None)
        .unwrap_or_else(|| Some(profile.aws_profile.trim().to_string()));
    let default_model = profile.default_model.clone();

    if region.is_empty() {
        return Ok(BedrockConnectionDiagnostic {
            ok: false,
            profile: profile_name,
            region,
            default_model,
            default_model_available: false,
            discovered_model_count: 0,
            message: "Amazon Bedrock AWS region missing".to_string(),
        });
    }

    match self.refresh_bedrock_models(&merged).await {
        Ok(models) => {
            let default_model_available = default_model
                .as_ref()
                .is_some_and(|model| models.iter().any(|candidate| candidate == model));
            let ok = default_model_available || default_model.is_none();
            let message = if ok {
                "Bedrock credentials and model discovery succeeded".to_string()
            } else {
                format!(
                    "Bedrock credentials work, but default model `{}` was not discovered in region `{}`",
                    default_model.as_deref().unwrap_or(""),
                    region
                )
            };
            Ok(BedrockConnectionDiagnostic {
                ok,
                profile: profile_name,
                region,
                default_model,
                default_model_available,
                discovered_model_count: models.len(),
                message,
            })
        }
        Err(error) => Ok(BedrockConnectionDiagnostic {
            ok: false,
            profile: profile_name,
            region,
            default_model,
            default_model_available: false,
            discovered_model_count: 0,
            message: error.to_string(),
        }),
    }
}
```

- [ ] **Step 4: Add backend delegation**

In `crates/orchestration/src/backend/mod.rs`, update the API import if needed and add this method near `refresh_bedrock_models()`:

```rust
pub async fn check_bedrock_connection(
    &self,
    settings: &AppSettings,
) -> Result<crate::api::BedrockConnectionDiagnostic, BackendError> {
    self.settings.check_bedrock_connection(settings).await
}
```

- [ ] **Step 5: Verify orchestration tests**

Run:

```bash
cargo test -p orchestration bedrock_readiness_uses_migrated_legacy_profile -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/api.rs crates/orchestration/src/settings/facade.rs crates/orchestration/src/backend/mod.rs crates/orchestration/src/backend/tests.rs
git commit -m "feat: add Bedrock connection diagnostics backend"
```

---

### Task 4: Expose The Diagnostic Through Desktop IPC And UI Port

**Files:**
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/ui/src/lib/types/index.ts`
- Modify: `crates/ui/src/api.ts`
- Modify: `crates/ui/src/port.ts`

- [ ] **Step 1: Add the Tauri command**

In `crates/desktop/src/lib.rs`, add `BedrockConnectionDiagnostic` to the existing orchestration import:

```rust
use orchestration::{
    AppBackend, BackendError, BedrockConnectionDiagnostic, FileEditPreview, ProviderReadiness,
    ScheduleStatus,
};
```

Add this command after `refresh_bedrock_models()`:

```rust
/// Tauri command: Check Bedrock credentials, region, and default model availability.
#[tauri::command]
async fn check_bedrock_connection(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
) -> Result<BedrockConnectionDiagnostic, CommandError> {
    Ok(backend.check_bedrock_connection(&settings).await?)
}
```

Register it in the `tauri::generate_handler!` list:

```rust
check_bedrock_connection,
```

- [ ] **Step 2: Mirror the type in TypeScript**

Add this interface after `ProviderReadiness` in `crates/ui/src/lib/types/index.ts`:

```ts
export interface BedrockConnectionDiagnostic {
  ok: boolean;
  profile: string | null;
  region: string;
  defaultModel: string | null;
  defaultModelAvailable: boolean;
  discoveredModelCount: number;
  message: string;
}
```

- [ ] **Step 3: Add the API wrapper**

Update the type import in `crates/ui/src/api.ts` to include `BedrockConnectionDiagnostic`, then add:

```ts
export function checkBedrockConnection(settings: AppSettings) {
  return invoke<BedrockConnectionDiagnostic>("check_bedrock_connection", { settings });
}
```

- [ ] **Step 4: Add the UI port method**

In `crates/ui/src/port.ts`, import `checkBedrockConnection` from `./api`, import `BedrockConnectionDiagnostic` from `./lib/types`, and add this to `UiDesktopOutboundPort`:

```ts
checkBedrockConnection: (settings: AppSettings) => Promise<BedrockConnectionDiagnostic>;
```

Add this to `createTauriDesktopPort()`:

```ts
checkBedrockConnection: desktopApi.checkBedrockConnection,
```

- [ ] **Step 5: Run frontend typecheck**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Expected: PASS, or only pre-existing unrelated errors. If errors mention `BedrockConnectionDiagnostic`, fix the import/type spelling before continuing.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/lib.rs crates/ui/src/lib/types/index.ts crates/ui/src/api.ts crates/ui/src/port.ts
git commit -m "feat: expose Bedrock diagnostics over IPC"
```

---

### Task 5: Add Bedrock Diagnostics UI

**Files:**
- Modify: `crates/ui/src/context/AppContext.tsx`
- Modify: `crates/ui/src/context/AppProvider.tsx`
- Modify: `crates/ui/src/settings/ProvidersSection.tsx`
- Test: `crates/ui/src/settings/ProvidersSection.test.tsx`

- [ ] **Step 1: Extend app context types**

In `crates/ui/src/context/AppContext.tsx`, import `BedrockConnectionDiagnostic` and add these fields to `AppContextValue`:

```ts
bedrockDiagnostic: Accessor<BedrockConnectionDiagnostic | null>;
checkingBedrock: Accessor<boolean>;
handleCheckBedrockConnection: () => Promise<void>;
```

- [ ] **Step 2: Implement state and handler**

In `crates/ui/src/context/AppProvider.tsx`, import `BedrockConnectionDiagnostic` and add state near existing provider settings signals:

```ts
const [bedrockDiagnostic, setBedrockDiagnostic] =
  createSignal<BedrockConnectionDiagnostic | null>(null);
const [checkingBedrock, setCheckingBedrock] = createSignal(false);
```

Add this handler near `handleSaveSettings()`:

```ts
const handleCheckBedrockConnection = async () => {
  setCheckingBedrock(true);
  setBedrockDiagnostic(null);
  try {
    const diagnostic = await desktop.checkBedrockConnection(settings());
    setBedrockDiagnostic(diagnostic);
    if (!diagnostic.ok) {
      setError(diagnostic.message);
    }
  } catch (error) {
    setError(normalizeError(error));
  } finally {
    setCheckingBedrock(false);
  }
};
```

Add the new fields to the `value` object:

```ts
bedrockDiagnostic,
checkingBedrock,
handleCheckBedrockConnection,
```

- [ ] **Step 3: Render Bedrock diagnostic controls**

In `crates/ui/src/settings/ProvidersSection.tsx`, add this Bedrock-only block inside the Bedrock auth/connection area after the AWS profile input:

```tsx
<button
  type="button"
  class="secondary-button"
  disabled={ctx.checkingBedrock()}
  onClick={() => void ctx.handleCheckBedrockConnection()}
>
  {ctx.checkingBedrock() ? "Checking Bedrock..." : "Check Bedrock connection"}
</button>
<Show when={ctx.bedrockDiagnostic()}>
  {(diagnostic) => (
    <div class="diagnostic-list" classList={{ ready: diagnostic().ok }}>
      <div>
        <span>Profile</span>
        <strong>{diagnostic().profile ?? "default AWS chain"}</strong>
      </div>
      <div>
        <span>Region</span>
        <strong>{diagnostic().region || "missing"}</strong>
      </div>
      <div>
        <span>Default model</span>
        <strong>
          {diagnostic().defaultModel ?? "none"}
          {diagnostic().defaultModelAvailable ? " available" : " not found"}
        </strong>
      </div>
      <p>{diagnostic().message}</p>
    </div>
  )}
</Show>
```

- [ ] **Step 4: Add the UI test**

In `crates/ui/src/settings/ProvidersSection.test.tsx`, extend the stub context with:

```ts
bedrockDiagnostic: () => null,
checkingBedrock: () => false,
handleCheckBedrockConnection: vi.fn(),
```

Add this test:

```ts
test("shows Bedrock connection diagnostic action for Bedrock provider", () => {
  renderSection("bedrock");

  const button = Array.from(container.querySelectorAll("button")).find((candidate) =>
    candidate.textContent?.includes("Check Bedrock connection"),
  );

  expect(button).toBeTruthy();
});
```

Add this test:

```ts
test("renders Bedrock diagnostic result rows", () => {
  renderSection("bedrock", {
    bedrockDiagnostic: () => ({
      ok: false,
      profile: "openflow-bedrock",
      region: "ap-southeast-2",
      defaultModel: "amazon.nova-pro-v1:0",
      defaultModelAvailable: false,
      discoveredModelCount: 0,
      message: "Error when retrieving token from sso: Token has expired and refresh failed",
    }),
  });

  expect(container.textContent).toContain("openflow-bedrock");
  expect(container.textContent).toContain("ap-southeast-2");
  expect(container.textContent).toContain("Token has expired");
});
```

- [ ] **Step 5: Run focused UI tests**

Run:

```bash
npm --prefix crates/ui test -- ProvidersSection
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/context/AppContext.tsx crates/ui/src/context/AppProvider.tsx crates/ui/src/settings/ProvidersSection.tsx crates/ui/src/settings/ProvidersSection.test.tsx
git commit -m "feat: show Bedrock connection diagnostics in settings"
```

---

### Task 6: Update Bedrock Documentation

**Files:**
- Modify: `docs/architecture/provider-adapters.md`
- Modify: `docs/troubleshooting/README.md`

- [ ] **Step 1: Update provider adapter Bedrock notes**

In `docs/architecture/provider-adapters.md`, replace the SSO setup list with:

```markdown
### Bedrock with SSO

1. In Settings -> Providers -> Amazon Bedrock, set **AWS profile** to the profile name from `~/.aws/config`, such as `openflow-bedrock`.
2. Set **AWS region** to the region where the models are enabled, such as `ap-southeast-2`.
3. Run `aws sso login --profile openflow-bedrock` before starting a run. SSO tokens expire and must be refreshed.
4. Click **Check Bedrock connection** in Settings. A good result means OpenFlow can list Bedrock models in that region with that profile.
5. If the diagnostic says the default model is not found, click **Refresh from AWS** and choose a model from the discovered list.
```

- [ ] **Step 2: Add troubleshooting entry**

In `docs/troubleshooting/README.md`, add this section under provider troubleshooting:

```markdown
### Bedrock SSO token expired

Symptom: Bedrock fails with `Error when retrieving token from sso: Token has expired and refresh failed`.

Fix:

```bash
aws sso login --profile openflow-bedrock
aws sts get-caller-identity --profile openflow-bedrock
aws bedrock list-foundation-models --profile openflow-bedrock --region ap-southeast-2
```

Then open Settings -> Providers -> Amazon Bedrock and set:

- AWS profile: `openflow-bedrock`
- AWS region: `ap-southeast-2`
- Default model: one model returned by **Refresh from AWS**
```

- [ ] **Step 3: Verify docs links**

Run:

```bash
./scripts/verify.sh typos
```

Expected: PASS, or no new typo failures in the touched docs.

- [ ] **Step 4: Commit**

```bash
git add docs/architecture/provider-adapters.md docs/troubleshooting/README.md
git commit -m "docs: document Bedrock SSO diagnostics"
```

---

### Task 7: Final Verification

**Files:**
- No new files.
- Verify all touched code paths.

- [ ] **Step 1: Run provider/settings tests**

Run:

```bash
cargo test -p orchestration settings::model::tests::normalized_migrates_legacy_bedrock_api_key_to_aws_profile settings::provider::tests::provider_env_var_names_include_bedrock_profile_and_region -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run orchestration package tests**

Run:

```bash
cargo test -p orchestration --lib
```

Expected: PASS.

- [ ] **Step 3: Run UI checks**

Run:

```bash
npm --prefix crates/ui run typecheck
npm --prefix crates/ui test -- ProvidersSection
```

Expected: PASS.

- [ ] **Step 4: Run architecture check**

Run:

```bash
./scripts/check-architecture.sh
```

Expected: PASS.

- [ ] **Step 5: Run full verification gate**

Run:

```bash
./scripts/verify.sh
```

Expected: PASS. If it fails in unrelated areas, capture the failing step and rerun the narrow command printed by `verify.sh` before reporting.

---

## Manual Acceptance On This Work Machine

After implementation:

```bash
aws sso login --profile openflow-bedrock
aws sts get-caller-identity --profile openflow-bedrock
aws bedrock list-foundation-models --profile openflow-bedrock --region ap-southeast-2 --by-output-modality TEXT --by-inference-type ON_DEMAND
```

Then in OpenFlow:

1. Open Settings -> Providers.
2. Select Amazon Bedrock.
3. Confirm AWS profile is `openflow-bedrock`.
4. Confirm AWS region is `ap-southeast-2`.
5. Click **Check Bedrock connection**.
6. Expected result: diagnostic shows the profile, region, a discovered model count greater than zero, and whether `amazon.nova-pro-v1:0` is available.
7. If the default model is not available, click **Refresh from AWS**, select one discovered model, save settings, and run a one-node workflow.

## Self-Review

- Spec coverage: The plan fixes the immediate old-settings profile bug, captures missing `AWS_REGION` env resolution, adds a Bedrock-specific diagnostic path, exposes it in the UI, and documents the SSO workflow.
- Placeholder scan: No placeholder markers or vague implementation-only steps remain.
- Type consistency: Rust DTO fields use `snake_case`; TypeScript mirrors the serialized `camelCase` fields from `#[serde(rename_all = "camelCase")]`.
