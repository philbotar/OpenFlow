use providers::{
    builtin_provider_specs, provider_spec, ProviderId, ProviderKind, ProviderSpec,
    ReasoningEffortOption, WireApi,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type ProviderTransport = WireApi;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub display_name: String,
    pub base_url: String,
    pub transport: ProviderTransport,
    #[serde(default = "default_responses_path")]
    pub responses_path: String,
    #[serde(default = "default_chat_completions_path")]
    pub chat_completions_path: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    pub known_models: Vec<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub aws_profile: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub aws_region: String,
    /// Optional shell command whose stdout is `aws configure export-credentials`
    /// JSON; when set it supplies explicit credentials and the SDK default
    /// credential chain is skipped entirely.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub aws_credential_command: String,
    #[serde(default)]
    pub editable: bool,
    #[serde(skip)]
    pub new_model_input: String,
    #[serde(
        default,
        rename = "reasoningEffortOptions",
        alias = "reasoning_effort_options"
    )]
    pub reasoning_effort_options: Vec<ReasoningEffortOption>,
    #[serde(
        default,
        rename = "defaultReasoningBudgetTokens",
        alias = "default_reasoning_budget_tokens"
    )]
    pub default_reasoning_budget_tokens: BTreeMap<String, u32>,
    #[serde(
        default,
        rename = "defaultReasoningEffort",
        alias = "default_reasoning_effort"
    )]
    pub default_reasoning_effort: Option<String>,
    /// Per-model context window sizes (in tokens) for the bubble indicator.
    /// Users can override or extend the bundled defaults here.
    #[serde(default)]
    pub context_window_sizes: BTreeMap<String, u32>,
}

fn default_responses_path() -> String {
    "v1/responses".to_string()
}

fn default_chat_completions_path() -> String {
    "v1/chat/completions".to_string()
}

const fn default_request_timeout_secs() -> u64 {
    300
}

impl ProviderProfile {
    #[must_use]
    pub fn from_spec(spec: &ProviderSpec) -> Self {
        let (transport, responses_path, chat_completions_path, base_url, aws_region) =
            match spec.kind {
                ProviderKind::OpenAiCompatible(openai) => (
                    openai.default_wire_api,
                    openai.responses_path.to_string(),
                    openai.chat_completions_path.to_string(),
                    spec.default_base_url.to_string(),
                    String::new(),
                ),
                ProviderKind::Anthropic(_) => (
                    ProviderTransport::ChatCompletions,
                    default_responses_path(),
                    default_chat_completions_path(),
                    spec.default_base_url.to_string(),
                    String::new(),
                ),
                ProviderKind::Bedrock(bedrock) => (
                    ProviderTransport::ChatCompletions,
                    default_responses_path(),
                    default_chat_completions_path(),
                    String::new(),
                    bedrock.default_region.to_string(),
                ),
            };
        Self {
            display_name: spec.display_name.to_string(),
            base_url,
            transport,
            responses_path,
            chat_completions_path,
            request_timeout_secs: default_request_timeout_secs(),
            known_models: spec
                .default_models
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
            default_model: Some(spec.default_model.to_string()),
            api_key: String::new(),
            aws_profile: String::new(),
            aws_region,
            aws_credential_command: String::new(),
            editable: spec.editable,
            new_model_input: String::new(),
            reasoning_effort_options: spec.default_reasoning_effort_options(),
            default_reasoning_budget_tokens: Self::default_budget_tokens_for_spec(spec),
            default_reasoning_effort: None,
            context_window_sizes: crate::settings::default_context_window_sizes(),
        }
    }

    #[must_use]
    pub fn compatible_default() -> Self {
        provider_spec(&ProviderId::from("custom_openai_compatible"))
            .map(Self::from_spec)
            .unwrap_or_else(|| {
                Self::fallback("custom_openai_compatible", "Custom OpenAI-compatible API")
            })
    }

    fn fallback(_id: &str, display_name: &str) -> Self {
        Self {
            display_name: display_name.to_string(),
            base_url: "https://api.openai.com".to_string(),
            transport: ProviderTransport::Responses,
            responses_path: default_responses_path(),
            chat_completions_path: default_chat_completions_path(),
            request_timeout_secs: default_request_timeout_secs(),
            known_models: vec!["gpt-4o-mini".to_string()],
            default_model: Some("gpt-4o-mini".to_string()),
            api_key: String::new(),
            aws_profile: String::new(),
            aws_region: String::new(),
            aws_credential_command: String::new(),
            editable: false,
            new_model_input: String::new(),
            reasoning_effort_options: Vec::new(),
            default_reasoning_budget_tokens: BTreeMap::new(),
            default_reasoning_effort: None,
            context_window_sizes: crate::settings::default_context_window_sizes(),
        }
    }

    fn normalize(&mut self, spec: Option<&ProviderSpec>) {
        if let Some(spec) = spec {
            if self.display_name.trim().is_empty() {
                self.display_name = spec.display_name.to_string();
            }
            match spec.kind {
                ProviderKind::Bedrock(bedrock) => {
                    let legacy_region = self.base_url.trim();
                    if self.aws_region.trim().is_empty() && !legacy_region.is_empty() {
                        self.aws_region = legacy_region.to_string();
                    }
                    if self.aws_region.trim().is_empty() {
                        self.aws_region = bedrock.default_region.to_string();
                    }
                    self.base_url.clear();
                }
                ProviderKind::OpenAiCompatible(_) | ProviderKind::Anthropic(_) => {
                    if self.base_url.trim().is_empty() {
                        self.base_url = spec.default_base_url.to_string();
                    }
                    self.aws_region.clear();
                    self.aws_credential_command.clear();
                }
            }
            if self.known_models.is_empty() {
                self.known_models = spec
                    .default_models
                    .iter()
                    .map(|model| (*model).to_string())
                    .collect();
            }
            if self.default_model.is_none() {
                self.default_model = Some(spec.default_model.to_string());
            }
            self.editable = spec.editable;
            if self.reasoning_effort_options.is_empty() {
                self.reasoning_effort_options = spec.default_reasoning_effort_options();
            }
            if self.default_reasoning_budget_tokens.is_empty() {
                self.default_reasoning_budget_tokens = Self::default_budget_tokens_for_spec(spec);
            }
        }
        self.new_model_input.clear();
    }

    /// Build the default budget token map for a provider spec.
    #[must_use]
    fn default_budget_tokens_for_spec(spec: &ProviderSpec) -> BTreeMap<String, u32> {
        match spec.kind {
            ProviderKind::Anthropic(_) | ProviderKind::Bedrock(_) => {
                let mut map = BTreeMap::new();
                map.insert("low".to_string(), 10_240);
                map.insert("medium".to_string(), 40_960);
                map.insert("high".to_string(), 59_000);
                map
            }
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSettings {
    #[serde(default = "default_lsp_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub format_on_write: bool,
    #[serde(default)]
    pub diagnostics_on_write: bool,
    #[serde(default = "default_lsp_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_lsp_enabled() -> bool {
    true
}

fn default_lsp_timeout_ms() -> u64 {
    5_000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
    #[serde(default = "default_true")]
    pub discover_external: bool,
    #[serde(default)]
    pub disabled_discovered_ids: Vec<String>,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            discover_external: true,
            disabled_discovered_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub display_name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for LspSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            format_on_write: false,
            diagnostics_on_write: false,
            timeout_ms: default_lsp_timeout_ms(),
        }
    }
}

impl LspSettings {
    fn apply_env_overrides(&mut self) {
        if matches!(
            std::env::var("PI_LSP_ENABLED").as_deref(),
            Ok("0") | Ok("false") | Ok("off")
        ) {
            self.enabled = false;
        }
        if matches!(
            std::env::var("PI_LSP_FORMAT_ON_WRITE").as_deref(),
            Ok("1") | Ok("true") | Ok("on")
        ) {
            self.format_on_write = true;
        }
        if matches!(
            std::env::var("PI_LSP_DIAGNOSTICS_ON_WRITE").as_deref(),
            Ok("1") | Ok("true") | Ok("on")
        ) {
            self.diagnostics_on_write = true;
        }
        if let Ok(value) = std::env::var("PI_LSP_TIMEOUT_MS") {
            if let Ok(timeout) = value.parse() {
                self.timeout_ms = timeout;
            }
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let mut settings = Self::default();
        settings.apply_env_overrides();
        settings
    }

    #[must_use]
    pub fn runtime(&self) -> Self {
        let mut settings = self.clone();
        settings.apply_env_overrides();
        settings
    }

    #[must_use]
    pub fn writethrough_active(&self) -> bool {
        self.enabled && (self.format_on_write || self.diagnostics_on_write)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalDiagnosticsSettings {
    #[serde(default)]
    pub debug_output: bool,
}

/// search-cli providers that accept API keys, in settings-page display order.
pub const SEARCH_KEY_PROVIDERS: &[&str] = &[
    "brave",
    "serper",
    "exa",
    "jina",
    "linkup",
    "firecrawl",
    "tavily",
    "perplexity",
    "serpapi",
    "browserless",
    "xai",
    "parallel",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Explicit path to the search-cli binary. Empty means resolve from PATH
    /// plus common install locations (GUI launches get a minimal PATH).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub binary_path: String,
    /// Provider id (e.g. "brave") -> API key. Injected as
    /// `SEARCH_KEYS_<PROVIDER>` env vars when spawning the binary.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            binary_path: String::new(),
            keys: BTreeMap::new(),
        }
    }
}

impl SearchSettings {
    /// True when at least one key is available to the spawned process,
    /// either saved in settings or already present in the environment.
    #[must_use]
    pub fn has_configured_keys(&self) -> bool {
        if self.keys.values().any(|key| !key.trim().is_empty()) {
            return true;
        }
        SEARCH_KEY_PROVIDERS.iter().any(|provider| {
            let upper = provider.to_uppercase();
            std::env::var(format!("{upper}_API_KEY")).is_ok_and(|v| !v.trim().is_empty())
                || std::env::var(format!("SEARCH_KEYS_{upper}")).is_ok_and(|v| !v.trim().is_empty())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub active_provider: ProviderId,
    pub providers: BTreeMap<ProviderId, ProviderProfile>,
    #[serde(default)]
    pub skill_search_paths: Vec<String>,
    #[serde(default)]
    pub lsp: LspSettings,
    #[serde(default)]
    pub mcp: McpSettings,
    #[serde(default)]
    pub local_diagnostics: LocalDiagnosticsSettings,
    #[serde(default)]
    pub search: SearchSettings,
}

fn migrate_bedrock_legacy_profile(profile: &mut ProviderProfile) {
    let legacy_profile = profile.api_key.trim();
    if profile.aws_profile.trim().is_empty() && !legacy_profile.is_empty() {
        profile.aws_profile = legacy_profile.to_string();
    }
    profile.api_key.clear();
}

impl AppSettings {
    #[must_use]
    pub fn active_profile(&self) -> &ProviderProfile {
        self.providers
            .get(&self.active_provider)
            .expect("active provider profile exists")
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        let mut copy = self.clone();
        for profile in copy.providers.values_mut() {
            profile.api_key.clear();
        }
        for key in copy.search.keys.values_mut() {
            key.clear();
        }
        copy
    }

    pub(crate) fn normalized(mut self) -> Self {
        for spec in builtin_provider_specs() {
            let id = ProviderId::from(spec.id);
            self.providers
                .entry(id)
                .or_insert_with(|| ProviderProfile::from_spec(spec));
        }
        let ids = self.providers.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let spec = provider_spec(&id);
            if let Some(profile) = self.providers.get_mut(&id) {
                profile.normalize(spec);
            }
        }
        if !self.providers.contains_key(&self.active_provider) {
            self.active_provider = ProviderId::from("openai");
        }
        if let Some(profile) = self.providers.get_mut(&ProviderId::from("bedrock")) {
            migrate_bedrock_legacy_profile(profile);
        }
        self
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        let providers = builtin_provider_specs()
            .iter()
            .map(|spec| (ProviderId::from(spec.id), ProviderProfile::from_spec(spec)))
            .collect::<BTreeMap<_, _>>();
        Self {
            active_provider: ProviderId::from("openai"),
            providers,
            skill_search_paths: Vec::new(),
            lsp: LspSettings::default(),
            mcp: McpSettings::default(),
            local_diagnostics: LocalDiagnosticsSettings::default(),
            search: SearchSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub fn merge_preserved_api_keys(incoming: &mut AppSettings, existing: &AppSettings) {
    for (id, profile) in &mut incoming.providers {
        if profile.api_key.trim().is_empty() {
            if let Some(existing_profile) = existing.providers.get(id) {
                profile.api_key = existing_profile.api_key.clone();
            }
        }
    }
    for (provider, key) in &existing.search.keys {
        let entry = incoming.search.keys.entry(provider.clone()).or_default();
        if entry.trim().is_empty() {
            *entry = key.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn normalized_clears_bedrock_api_key() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .api_key = "legacy-profile-as-key".to_string();

        let normalized = settings.normalized();

        assert!(normalized
            .providers
            .get(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .api_key
            .is_empty());
    }

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

    #[test]
    fn bedrock_default_uses_aws_region_not_base_url() {
        let settings = AppSettings::default();
        let profile = settings
            .providers
            .get(&ProviderId::from("bedrock"))
            .expect("bedrock profile");

        assert_eq!(profile.aws_region, "us-east-1");
        assert!(profile.base_url.is_empty());
    }

    #[test]
    fn normalized_migrates_legacy_bedrock_base_url_to_aws_region() {
        let mut settings = AppSettings::default();
        let profile = settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile");
        profile.aws_region.clear();
        profile.base_url = " ap-southeast-2 ".to_string();

        let normalized = settings.normalized();
        let profile = normalized
            .providers
            .get(&ProviderId::from("bedrock"))
            .expect("bedrock profile");

        assert_eq!(profile.aws_region, "ap-southeast-2");
        assert!(profile.base_url.is_empty());
    }

    #[test]
    fn provider_profile_serde_roundtrip_with_reasoning_effort_options() {
        let profile = ProviderProfile {
            reasoning_effort_options: vec![ReasoningEffortOption {
                value: "adaptive".to_string(),
                label: "Adaptive".to_string(),
                uses_budget_tokens: false,
            }],
            default_reasoning_budget_tokens: {
                let mut m = BTreeMap::new();
                m.insert("low".to_string(), 10_240);
                m
            },
            default_reasoning_effort: Some("adaptive".to_string()),
            ..ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap())
        };
        let value = serde_json::to_value(&profile).unwrap();
        assert!(value["reasoningEffortOptions"].is_array());
        assert_eq!(value["reasoningEffortOptions"][0]["value"], "adaptive");
        assert_eq!(value["defaultReasoningBudgetTokens"]["low"], 10_240);
        assert_eq!(value["defaultReasoningEffort"], "adaptive");
        let back: ProviderProfile = serde_json::from_value(value).unwrap();
        assert_eq!(back.reasoning_effort_options.len(), 1);
        assert_eq!(back.default_reasoning_effort.as_deref(), Some("adaptive"));
        assert_eq!(
            back.default_reasoning_budget_tokens.get("low"),
            Some(&10_240)
        );
    }

    #[test]
    fn provider_profile_backfills_from_spec_when_empty() {
        // Simulate a profile saved before reasoning effort fields existed
        let value = serde_json::json!({
            "display_name": "Anthropic",
            "base_url": "https://api.anthropic.com",
            "transport": "chat_completions",
            "responses_path": "v1/responses",
            "chat_completions_path": "v1/chat/completions",
            "known_models": ["claude-3-5-sonnet-latest"],
            "default_model": "claude-3-5-sonnet-latest",
            "api_key": "",
            "editable": false
        });
        let mut profile: ProviderProfile = serde_json::from_value(value).unwrap();
        assert_eq!(profile.request_timeout_secs, 300);
        assert!(profile.reasoning_effort_options.is_empty());
        assert!(profile.default_reasoning_budget_tokens.is_empty());

        let spec = provider_spec(&ProviderId::from("anthropic")).unwrap();
        profile.normalize(Some(spec));
        assert!(!profile.reasoning_effort_options.is_empty());
        assert_eq!(profile.reasoning_effort_options.len(), 5);
        assert_eq!(
            profile.default_reasoning_budget_tokens.get("high"),
            Some(&59_000)
        );
    }

    #[test]
    fn provider_profile_preserves_user_added_options() {
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        // Add a custom user option
        profile
            .reasoning_effort_options
            .push(ReasoningEffortOption {
                value: "custom".to_string(),
                label: "Custom".to_string(),
                uses_budget_tokens: true,
            });
        let original_len = profile.reasoning_effort_options.len();

        // Normalize should NOT overwrite user options
        let spec = provider_spec(&ProviderId::from("anthropic")).unwrap();
        profile.normalize(Some(spec));
        assert_eq!(profile.reasoning_effort_options.len(), original_len);
    }

    #[test]
    fn mcp_settings_default_enables_external_discovery() {
        assert!(McpSettings::default().discover_external);
    }

    #[test]
    fn app_settings_missing_mcp_key_enables_external_discovery() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("mcp");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert!(parsed.mcp.discover_external);
    }

    #[test]
    fn app_settings_default_local_diagnostics_disabled() {
        assert!(!AppSettings::default().local_diagnostics.debug_output);
    }

    #[test]
    fn app_settings_missing_local_diagnostics_defaults_disabled() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("local_diagnostics");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert!(!parsed.local_diagnostics.debug_output);
    }

    #[test]
    fn mcp_discovery_settings_round_trip() {
        let settings = AppSettings {
            mcp: McpSettings {
                servers: vec![],
                discover_external: true,
                disabled_discovered_ids: vec!["playwright".into()],
            },
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(parsed.mcp.discover_external);
        assert_eq!(parsed.mcp.disabled_discovered_ids, ["playwright"]);
    }

    #[test]
    fn mcp_settings_round_trip() {
        let settings = AppSettings {
            mcp: McpSettings {
                servers: vec![McpServerConfig {
                    id: "github".into(),
                    display_name: "GitHub".into(),
                    command: "npx".into(),
                    args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
                    env: Default::default(),
                    enabled: true,
                }],
                ..McpSettings::default()
            },
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mcp.servers[0].id, "github");
    }

    #[test]
    fn search_settings_default_is_enabled_with_no_keys() {
        let settings = SearchSettings::default();
        assert!(settings.enabled);
        assert!(settings.binary_path.is_empty());
        assert!(settings.keys.is_empty());
    }

    #[test]
    fn app_settings_missing_search_key_parses_to_default() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("search");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.search, SearchSettings::default());
    }

    #[test]
    fn search_settings_round_trip_uses_camel_case() {
        let mut settings = AppSettings::default();
        settings.search.binary_path = "/opt/homebrew/bin/search".to_string();
        settings
            .search
            .keys
            .insert("brave".to_string(), "bk-123".to_string());
        let value = serde_json::to_value(&settings).unwrap();
        assert_eq!(value["search"]["binaryPath"], "/opt/homebrew/bin/search");
        assert_eq!(value["search"]["keys"]["brave"], "bk-123");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.search, settings.search);
    }

    #[test]
    fn redacted_clears_search_keys_but_keeps_entries() {
        let mut settings = AppSettings::default();
        settings
            .search
            .keys
            .insert("brave".to_string(), "bk-123".to_string());
        let redacted = settings.redacted();
        assert_eq!(
            redacted.search.keys.get("brave").map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn merge_preserved_api_keys_restores_search_keys() {
        let mut existing = AppSettings::default();
        existing
            .search
            .keys
            .insert("brave".to_string(), "bk-123".to_string());
        existing
            .search
            .keys
            .insert("exa".to_string(), "ek-456".to_string());

        let mut incoming = AppSettings::default();
        incoming
            .search
            .keys
            .insert("brave".to_string(), String::new());

        merge_preserved_api_keys(&mut incoming, &existing);
        assert_eq!(
            incoming.search.keys.get("brave").map(String::as_str),
            Some("bk-123")
        );
        assert_eq!(
            incoming.search.keys.get("exa").map(String::as_str),
            Some("ek-456")
        );
    }

    #[test]
    fn has_configured_keys_detects_settings_keys() {
        let mut settings = SearchSettings::default();
        assert!(!settings.keys.values().any(|key| !key.trim().is_empty()));
        settings
            .keys
            .insert("brave".to_string(), " bk ".to_string());
        assert!(settings.has_configured_keys());
    }
}
