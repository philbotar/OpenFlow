# Bedrock AWS Credential Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `aws_credential_command` setting to the Bedrock provider profile — a user-supplied shell command (e.g. `aws configure export-credentials --profile bedrock`) whose JSON output supplies explicit AWS credentials, bypassing the Rust SDK's default credential chain entirely.

**Architecture:** The setting is a new `String` field on `ProviderProfile` (mirrors the existing `aws_profile` field exactly). It threads through `resolve_provider_config` → `BedrockConfig` → `load_aws_sdk_config` as a third `Option<&str>` parameter. When set, `load_aws_sdk_config` runs the command via the platform shell, parses the AWS-CLI `export-credentials` JSON shape with the **already-existing** `parse_cli_export_credentials` function, and builds the SDK config with explicit `Credentials` — no chain probe. On any failure it falls through to the existing behavior (chain probe, then hardcoded `aws configure export-credentials` fallback, then SSO login retry). Rig is unaffected: `build_bedrock` in `rig_adapter/model.rs` already constructs the rig client from an `aws_sdk_bedrockruntime::Client` built from this SdkConfig, so explicit credentials flow into rig for free.

**Tech Stack:** Rust (aws-config, aws-sdk-bedrockruntime, tokio::process, serde), SolidJS + vitest for the settings UI.

**What was deliberately skipped (ponytail):**
- No env-var fallback for the command (settings key only — add when someone asks).
- No credential caching/expiry handling — the SdkConfig is already rebuilt per invoke and creds are used immediately (same as the existing CLI fallback, see the `ponytail:` comment at `aws_runtime.rs:73`).
- `AuthConfig::AwsCredentials` is NOT extended — only the adapter-side `BedrockConfig` feeds the SDK; the auth enum's profile/region are informational.
- Command runs via `sh -c` / `cmd /C` so users can write a plain command string with args and pipes.

---

## File Map

| File | Change |
|---|---|
| `crates/orchestration/src/settings/model.rs` | Add `aws_credential_command: String` to `ProviderProfile` (serde default, skip-if-empty); clear it for non-Bedrock providers in `normalize`; populate in `from_spec`/`fallback` constructors. |
| `crates/orchestration/src/settings/provider.rs` | Resolve trimmed command from profile into `BedrockConfig`. |
| `crates/providers/src/client.rs` | Add `aws_credential_command: Option<String>` to `BedrockConfig`. |
| `crates/providers/src/aws_runtime.rs` | New `custom_command_credentials` fn; new third param on `load_aws_sdk_config` that short-circuits the chain. |
| `crates/providers/src/bedrock_models.rs` | Thread param through `list_bedrock_foundation_models`, `verify_bedrock_credentials`, `bedrock_control_client`. |
| `crates/providers/src/rig_adapter/model.rs` | Pass `bedrock_config.aws_credential_command` in `build_bedrock`. |
| `crates/orchestration/src/settings/facade.rs` | Pass the field at the two call sites (lines ~189, ~225). |
| `crates/ui/src/lib/types/index.ts` | Add `aws_credential_command?: string` to `ProviderProfile`. |
| `crates/ui/src/lib/workflow/reasoning.ts` | Copy the field in `cloneProviderProfile` (it rebuilds the object; omission would silently drop the setting). |
| `crates/ui/src/settings/ProvidersSection.tsx` | Text input under the existing "AWS profile" field. |
| `crates/ui/src/settings/ProvidersSection.test.tsx` | One test: typing in the field updates settings. |

Settings JSON shape this produces (field lives on the bedrock provider profile, matching every other bedrock field — not a new top-level `bedrock` key):

```json
{
  "providers": {
    "bedrock": {
      "aws_region": "us-east-1",
      "aws_profile": "bedrock",
      "aws_credential_command": "aws configure export-credentials --profile bedrock"
    }
  }
}
```

---

### Task 1: Settings model field

**Files:**
- Modify: `crates/orchestration/src/settings/model.rs`
- Tests: same file, `mod tests`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/orchestration/src/settings/model.rs`:

```rust
#[test]
fn provider_profile_roundtrips_aws_credential_command() {
    let mut settings = AppSettings::default();
    settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile")
        .aws_credential_command =
        "aws configure export-credentials --profile bedrock".to_string();
    let json = serde_json::to_string(&settings).unwrap();
    let parsed: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed
            .providers
            .get(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_credential_command,
        "aws configure export-credentials --profile bedrock"
    );
}

#[test]
fn normalized_clears_credential_command_for_non_bedrock() {
    let mut settings = AppSettings::default();
    settings
        .providers
        .get_mut(&ProviderId::from("openai"))
        .expect("openai profile")
        .aws_credential_command = "aws configure export-credentials".to_string();
    let normalized = settings.normalized();
    assert!(normalized
        .providers
        .get(&ProviderId::from("openai"))
        .expect("openai profile")
        .aws_credential_command
        .is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p orchestration settings::model -- credential_command`
Expected: FAIL to compile — `aws_credential_command` field does not exist.

- [ ] **Step 3: Implement**

In `ProviderProfile` (after the `aws_region` field, ~line 27):

```rust
    /// Optional shell command whose stdout is `aws configure export-credentials`
    /// JSON; when set it supplies explicit credentials and the SDK default
    /// credential chain is skipped entirely.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub aws_credential_command: String,
```

Add `aws_credential_command: String::new(),` to both struct literals: `from_spec` (next to `aws_profile: String::new(),` ~line 104) and `fallback` (~line 134).

In `normalize`, inside the `ProviderKind::OpenAiCompatible(_) | ProviderKind::Anthropic(_)` arm (after `self.aws_region.clear();` ~line 165):

```rust
                    self.aws_credential_command.clear();
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p orchestration settings::model`
Expected: PASS (all existing model tests too).

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/settings/model.rs
git commit -m "feat(settings): add aws_credential_command to bedrock provider profile"
```

---

### Task 2: BedrockConfig field and resolution

**Files:**
- Modify: `crates/providers/src/client.rs` (~line 34)
- Modify: `crates/orchestration/src/settings/provider.rs`
- Tests: `crates/orchestration/src/settings/provider.rs` `mod tests`

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/orchestration/src/settings/provider.rs`. Model it on the existing `bedrock_stored_aws_profile_beats_env` test (~line 527) — same setup helpers, same `resolve_provider_config` call shape used by neighboring tests:

```rust
#[test]
fn bedrock_credential_command_flows_to_adapter_config() {
    let mut settings = AppSettings::default();
    settings.active_provider = ProviderId::from("bedrock");
    let profile = settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile");
    profile.aws_credential_command =
        "  aws configure export-credentials --profile bedrock  ".to_string();

    let config = resolve_provider_config(&settings, None, &ProviderEnv::default())
        .expect("provider config");

    let ProviderAdapterConfig::Bedrock(bedrock) = config.adapter else {
        panic!("expected bedrock adapter");
    };
    assert_eq!(
        bedrock.aws_credential_command.as_deref(),
        Some("aws configure export-credentials --profile bedrock")
    );
}
```

Note: check how neighboring tests construct `ProviderEnv` (some use a helper or `ProviderEnv::from_pairs`-style constructor around line 380). Use the same constructor they use, not necessarily `Default`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration settings::provider -- credential_command`
Expected: FAIL to compile — no field `aws_credential_command` on `BedrockConfig`.

- [ ] **Step 3: Implement**

`crates/providers/src/client.rs` (~line 34):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockConfig {
    pub region: String,
    pub aws_profile: Option<String>,
    pub aws_credential_command: Option<String>,
}
```

`crates/orchestration/src/settings/provider.rs`, in `resolve_provider_config`, `ProviderKind::Bedrock(_)` arm (~line 126):

```rust
            ProviderAdapterConfig::Bedrock(BedrockConfig {
                region,
                aws_profile: bedrock_aws_profile,
                aws_credential_command: first_trimmed_string([Some(
                    profile.aws_credential_command.as_str(),
                )]),
            })
```

Fix any other `BedrockConfig { .. }` literals the compiler reports (tests in provider.rs construct it around lines 401/424/448) by adding `aws_credential_command: None`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p orchestration settings::provider && cargo check -p providers`
Expected: PASS / clean check (facade.rs call sites don't break yet — they only read `.region` and `.aws_profile`).

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/client.rs crates/orchestration/src/settings/provider.rs
git commit -m "feat(providers): thread aws_credential_command into BedrockConfig"
```

---

### Task 3: Shell-out credential source in aws_runtime

**Files:**
- Modify: `crates/providers/src/aws_runtime.rs`
- Tests: same file, `mod tests`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/providers/src/aws_runtime.rs`:

```rust
    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::expect_used)]
    async fn custom_command_credentials_parses_shell_output() {
        let creds = custom_command_credentials(
            r#"printf '{"AccessKeyId":"AKIA2","SecretAccessKey":"s","SessionToken":"t"}'"#,
        )
        .await
        .expect("credentials");
        assert_eq!(creds.access_key_id(), "AKIA2");
        assert_eq!(creds.session_token(), Some("t"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn custom_command_credentials_none_on_failure() {
        assert!(custom_command_credentials("exit 1").await.is_none());
        assert!(custom_command_credentials("printf 'not json'").await.is_none());
    }
```

If the file doesn't already have `tokio::test` available as a dev-dependency of `providers`, check `crates/providers/Cargo.toml` — the crate already depends on tokio for `cli_export_credentials`; add `tokio = { version = "1", features = ["macros", "rt"] }` under `[dev-dependencies]` only if the test attribute fails to resolve.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p providers aws_runtime -- custom_command`
Expected: FAIL to compile — `custom_command_credentials` not found.

- [ ] **Step 3: Implement**

In `crates/providers/src/aws_runtime.rs`, after `cli_export_credentials` (~line 96):

```rust
/// Runs a user-configured shell command and parses its stdout as
/// `aws configure export-credentials` JSON. This is how users sidestep the
/// Rust SDK credential chain entirely (its IAM Identity Center support is
/// partial); same pattern Claude Code uses for `awsAuthRefresh`.
async fn custom_command_credentials(command_line: &str) -> Option<Credentials> {
    #[cfg(windows)]
    let mut command = {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", command_line]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", command_line]);
        c
    };
    command.kill_on_drop(true);
    // ponytail: 30s cap, same budget as cli_export_credentials
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_cli_export_credentials(&output.stdout)
}
```

Change `load_aws_sdk_config` (~line 31) to accept and honor the command:

```rust
pub(crate) async fn load_aws_sdk_config(
    region: &str,
    profile: Option<&str>,
    credential_command: Option<&str>,
) -> aws_config::SdkConfig {
    ensure_process_home_env();
    let trimmed_region = region.trim();
    // ponytail: user command wins outright — skip the chain probe entirely
    if let Some(command_line) = credential_command.map(str::trim).filter(|c| !c.is_empty()) {
        if let Some(credentials) = custom_command_credentials(command_line).await {
            return aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_config::Region::new(trimmed_region.to_string()))
                .credentials_provider(credentials)
                .load()
                .await;
        }
        // command failed → fall through to the default chain + built-in fallbacks
    }
    let profile_name = sanitize_profile(profile);
    // ... rest of the existing function body unchanged ...
```

(Only the signature and the new `if let` block at the top change; everything from `let mut loader = ...` down stays as-is.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p providers aws_runtime`
Expected: new tests PASS. `cargo check -p providers` will FAIL at the three `load_aws_sdk_config` call sites — that's Task 4; commit only aws_runtime.rs if check fails, or fold Task 4's call-site edits into this commit if you prefer one compiling commit. **Prefer the single compiling commit: do Task 4 Steps 1–2 now and commit together.**

---

### Task 4: Thread the parameter through all call sites

**Files:**
- Modify: `crates/providers/src/bedrock_models.rs` (lines 14–18, 35–45, 94–108)
- Modify: `crates/providers/src/rig_adapter/model.rs` (~line 120)
- Modify: `crates/orchestration/src/settings/facade.rs` (~lines 189, 225)

- [ ] **Step 1: Update providers crate call sites**

`bedrock_models.rs` — add `aws_credential_command: Option<&str>` as a third parameter to all three functions and forward it:

```rust
pub async fn list_bedrock_foundation_models(
    region: &str,
    aws_profile: Option<&str>,
    aws_credential_command: Option<&str>,
) -> Result<Vec<String>, AgentError> {
    let client = bedrock_control_client(region, aws_profile, aws_credential_command).await?;
    // ... unchanged
```

```rust
pub async fn verify_bedrock_credentials(
    region: &str,
    aws_profile: Option<&str>,
    aws_credential_command: Option<&str>,
) -> Result<String, AgentError> {
    // ... unchanged until:
    let config = load_aws_sdk_config(trimmed_region, aws_profile, aws_credential_command).await;
    // ... unchanged
```

```rust
async fn bedrock_control_client(
    region: &str,
    aws_profile: Option<&str>,
    aws_credential_command: Option<&str>,
) -> Result<BedrockControlClient, AgentError> {
    // ... unchanged until:
    let shared = load_aws_sdk_config(
        trimmed_region,
        aws_profile.map(str::trim).filter(|value| !value.is_empty()),
        aws_credential_command,
    )
    .await;
```

`rig_adapter/model.rs` `build_bedrock` (~line 120):

```rust
    let sdk_config = crate::aws_runtime::load_aws_sdk_config(
        &bedrock_config.region,
        bedrock_config.aws_profile.as_deref(),
        bedrock_config.aws_credential_command.as_deref(),
    )
    .await;
```

- [ ] **Step 2: Update orchestration facade call sites**

`facade.rs` line ~189:

```rust
            return list_bedrock_foundation_models(
                &bedrock.region,
                bedrock.aws_profile.as_deref(),
                bedrock.aws_credential_command.as_deref(),
            )
            .await
            .map_err(map_agent_error_to_backend);
```

`facade.rs` line ~225:

```rust
            return verify_bedrock_credentials(
                &bedrock.region,
                bedrock.aws_profile.as_deref(),
                bedrock.aws_credential_command.as_deref(),
            )
            .await
            .map_err(map_agent_error_to_backend);
```

- [ ] **Step 3: Full workspace check and test**

Run: `cargo test --workspace`
Expected: PASS. If `crates/providers/tests/rig_bedrock.rs` or any `BedrockConfig` literal elsewhere fails to compile, add `aws_credential_command: None` there.

- [ ] **Step 4: Commit**

```bash
git add crates/providers crates/orchestration
git commit -m "feat(bedrock): run user-configured credential command instead of SDK chain"
```

---

### Task 5: Settings UI

**Files:**
- Modify: `crates/ui/src/lib/types/index.ts` (~line 356)
- Modify: `crates/ui/src/lib/workflow/reasoning.ts` (~line 116)
- Modify: `crates/ui/src/settings/ProvidersSection.tsx` (~line 136, after the AWS profile label)
- Test: `crates/ui/src/settings/ProvidersSection.test.tsx`

- [ ] **Step 1: Write the failing test**

In `ProvidersSection.test.tsx`, copy the shape of the existing test that asserts `settings().providers.bedrock?.aws_profile` becomes `"work-profile"` after typing (~line 274) — same render helpers, same input-event pattern:

```tsx
it("updates the bedrock credential command when edited", async () => {
  // same setup as the aws_profile test above
  const input = screen.getByLabelText(/credential command/i);
  fireEvent.input(input, {
    target: { value: "aws configure export-credentials --profile bedrock" },
  });
  await waitForSettingsSave(); // use whatever awaiting helper the sibling test uses
  expect(settings().providers.bedrock?.aws_credential_command).toBe(
    "aws configure export-credentials --profile bedrock",
  );
});
```

(Adapt the helper/setup lines to match the neighboring `aws_profile` test verbatim — the file's existing patterns win over this sketch.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/ui && npx vitest run src/settings/ProvidersSection.test.tsx`
Expected: FAIL — no element matching /credential command/i.

- [ ] **Step 3: Implement**

`types/index.ts` — after `aws_region?: string;`:

```ts
  aws_credential_command?: string;
```

`reasoning.ts` `cloneProviderProfile` — after `aws_region: profile.aws_region,`:

```ts
    aws_credential_command: profile.aws_credential_command,
```

`ProvidersSection.tsx` — after the closing `</label>` of the AWS profile field (~line 136), inside the same Bedrock fallback block:

```tsx
              <label>
                <span>Credential command (optional)</span>
                <input
                  type="text"
                  class="text-input"
                  value={ctx.activeProfileMemo().aws_credential_command ?? ""}
                  placeholder="e.g. aws configure export-credentials --profile bedrock"
                  onInput={(event) =>
                    void ctx.updateSettings((draft) => {
                      activeProfile(draft).aws_credential_command = event.currentTarget.value;
                    })
                  }
                />
              </label>
```

Also update the explanatory `<p>` above the AWS profile field to mention the override, e.g. append: `To bypass the SDK credential chain entirely, set a credential command below — OpenFlow runs it and uses the exported keys directly.`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/ui && npx vitest run src/settings/ProvidersSection.test.tsx && npm run typecheck`
Expected: PASS, clean typecheck.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src
git commit -m "feat(ui): bedrock credential command setting"
```

---

### Task 6: Final verification

- [ ] **Step 1: Full test suite**

Run: `cargo test --workspace && cd crates/ui && npm test && npm run typecheck`
Expected: all PASS.

- [ ] **Step 2: Lint**

Run: `cargo clippy --workspace --all-targets` (repo uses strict clippy — fix any new warnings; note the crate's `#[allow(...)]` + `reason` idiom).
Expected: clean.

- [ ] **Step 3: Manual smoke (optional, needs AWS SSO setup)**

Set in the OpenFlow settings file (find it via `crates/desktop/src/commands/settings.rs` store path):

```json
"aws_credential_command": "aws configure export-credentials --profile bedrock"
```

Open Settings → Bedrock → "Test AWS connection". Expected: "AWS credentials loaded (access key id ends with …XXXX)" even when the SDK chain alone cannot resolve the SSO profile.
