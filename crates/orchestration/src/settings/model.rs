use crate::mcp::model::{
    McpConnection, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
    MCP_SERVER_RECORD_VERSION,
};
use providers::{
    builtin_provider_specs, provider_spec, CodexOAuthCredentials, ModelTransport, ProviderId,
    ProviderKind, ProviderSpec, ReasoningEffortOption, WireApi,
};
use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_transports: BTreeMap<String, ModelTransport>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_oauth: Option<CodexOAuthCredentials>,
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
    /// Whether this provider exposes a request-level faster service tier.
    #[serde(default, rename = "fastModeAvailable", alias = "fast_mode_available")]
    pub fast_mode_available: bool,
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
                ProviderKind::OpenAiCodex => (
                    ProviderTransport::Responses,
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
            model_transports: BTreeMap::new(),
            default_model: Some(spec.default_model.to_string()),
            api_key: String::new(),
            codex_oauth: None,
            aws_profile: String::new(),
            aws_region,
            aws_credential_command: String::new(),
            editable: spec.editable,
            new_model_input: String::new(),
            reasoning_effort_options: spec.default_reasoning_effort_options(),
            default_reasoning_budget_tokens: Self::default_budget_tokens_for_spec(spec),
            default_reasoning_effort: None,
            fast_mode_available: spec.supports_fast_mode(),
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
            model_transports: BTreeMap::new(),
            default_model: Some("gpt-4o-mini".to_string()),
            api_key: String::new(),
            codex_oauth: None,
            aws_profile: String::new(),
            aws_region: String::new(),
            aws_credential_command: String::new(),
            editable: false,
            new_model_input: String::new(),
            reasoning_effort_options: Vec::new(),
            default_reasoning_budget_tokens: BTreeMap::new(),
            default_reasoning_effort: None,
            fast_mode_available: false,
            context_window_sizes: crate::settings::default_context_window_sizes(),
        }
    }

    fn normalize(&mut self, spec: Option<&ProviderSpec>) {
        if let Some(spec) = spec {
            let has_legacy_codex_label =
                spec.id == "openai-codex" && self.display_name == "OpenAI Codex";
            if self.display_name.trim().is_empty() || has_legacy_codex_label {
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
                ProviderKind::OpenAiCompatible(_)
                | ProviderKind::OpenAiCodex
                | ProviderKind::Anthropic(_) => {
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
            for option in &mut self.reasoning_effort_options {
                if option.value == "none" && option.label == "Fast" {
                    option.label = "None".to_string();
                }
            }
            if self.default_reasoning_budget_tokens.is_empty() {
                self.default_reasoning_budget_tokens = Self::default_budget_tokens_for_spec(spec);
            }
            self.fast_mode_available = spec.supports_fast_mode();
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
    #[serde(default, deserialize_with = "deserialize_mcp_servers")]
    pub servers: Vec<McpServerRecord>,
    #[serde(default)]
    pub discover_external: bool,
    #[serde(default)]
    pub disabled_discovered_ids: Vec<String>,
    #[serde(default = "default_mcp_registry_base_url")]
    pub registry_base_url: String,
}

fn default_mcp_registry_base_url() -> String {
    crate::mcp::catalog::DEFAULT_MCP_REGISTRY_BASE_URL.to_string()
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            discover_external: false,
            disabled_discovered_ids: Vec::new(),
            registry_base_url: default_mcp_registry_base_url(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMcpServerConfig {
    id: String,
    display_name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    #[allow(
        dead_code,
        reason = "legacy value is intentionally not trusted during migration"
    )]
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum McpServerWire {
    Current(Box<McpServerRecord>),
    Legacy(LegacyMcpServerConfig),
}

fn deserialize_mcp_servers<'de, D>(deserializer: D) -> Result<Vec<McpServerRecord>, D::Error>
where
    D: Deserializer<'de>,
{
    let servers = Vec::<McpServerWire>::deserialize(deserializer)?;
    Ok(servers
        .into_iter()
        .map(|server| match server {
            McpServerWire::Current(record) => *record,
            McpServerWire::Legacy(legacy) => McpServerRecord {
                schema_version: MCP_SERVER_RECORD_VERSION,
                id: legacy.id,
                display_name: legacy.display_name,
                source: McpServerSource::Manual,
                install: McpInstall::External,
                connection: McpConnection::Stdio {
                    command: legacy.command,
                    args: legacy.args,
                    environment: legacy
                        .env
                        .into_iter()
                        .map(|(key, value)| (key, PersistedValue::Literal { value }))
                        .collect(),
                },
                trust: Default::default(),
                policy: Default::default(),
                enabled: false,
                install_history: None,
            },
        })
        .collect())
}

/// Compatibility name for desktop/UI seams while normalized records roll out.
pub type McpServerConfig = McpServerRecord;

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
            profile.codex_oauth = None;
        }
        for key in copy.search.keys.values_mut() {
            key.clear();
        }
        for server in &mut copy.mcp.servers {
            *server = server.redacted();
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

pub fn merge_preserved_secrets(incoming: &mut AppSettings, existing: &AppSettings) {
    for (id, profile) in &mut incoming.providers {
        if let Some(existing_profile) = existing.providers.get(id) {
            if profile.api_key.trim().is_empty() {
                profile.api_key = existing_profile.api_key.clone();
            }
            if profile.codex_oauth.is_none() {
                profile
                    .codex_oauth
                    .clone_from(&existing_profile.codex_oauth);
            }
        }
    }
    for (provider, key) in &existing.search.keys {
        let entry = incoming.search.keys.entry(provider.clone()).or_default();
        if entry.trim().is_empty() {
            *entry = key.clone();
        }
    }
    for incoming_server in &mut incoming.mcp.servers {
        if let Some(existing_server) = existing
            .mcp
            .servers
            .iter()
            .find(|server| server.id == incoming_server.id)
        {
            merge_preserved_mcp_env(incoming_server, existing_server);
        }
    }
}

pub(crate) fn merge_preserved_mcp_env(incoming: &mut McpServerConfig, existing: &McpServerConfig) {
    match (&mut incoming.connection, &existing.connection) {
        (
            McpConnection::Stdio {
                environment: incoming,
                ..
            },
            McpConnection::Stdio {
                environment: existing,
                ..
            },
        )
        | (
            McpConnection::StreamableHttp {
                headers: incoming, ..
            },
            McpConnection::StreamableHttp {
                headers: existing, ..
            },
        )
        | (
            McpConnection::LegacySse {
                headers: incoming, ..
            },
            McpConnection::LegacySse {
                headers: existing, ..
            },
        ) => merge_preserved_mcp_values(incoming, existing),
        _ => {}
    }
}

fn merge_preserved_mcp_values(
    incoming: &mut BTreeMap<String, PersistedValue>,
    existing: &BTreeMap<String, PersistedValue>,
) {
    for (key, incoming_value) in incoming {
        if let PersistedValue::Secret {
            secret_ref,
            resolved_value,
        } = incoming_value
        {
            if resolved_value.is_none() {
                if let Some(PersistedValue::Secret {
                    secret_ref: existing_ref,
                    resolved_value: existing_value,
                }) = existing.get(key)
                {
                    if existing_ref == secret_ref {
                        resolved_value.clone_from(existing_value);
                    }
                }
            }
            continue;
        }
        let is_empty_literal = matches!(
            incoming_value,
            PersistedValue::Literal { value } if value.trim().is_empty()
        );
        if is_empty_literal {
            if let Some(existing_value) = existing.get(key) {
                incoming_value.clone_from(existing_value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp_stdio_server(
        id: &str,
        display_name: &str,
        command: &str,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        enabled: bool,
    ) -> McpServerConfig {
        let mut server = McpServerRecord::new(
            id,
            display_name,
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: command.to_string(),
                args,
                environment: env
                    .into_iter()
                    .map(|(key, value)| (key, PersistedValue::Literal { value }))
                    .collect(),
            },
        );
        server.enabled = enabled;
        server
    }

    fn codex_credentials() -> CodexOAuthCredentials {
        CodexOAuthCredentials {
            access_token: "access-sentinel".to_string(),
            refresh_token: "refresh-sentinel".to_string(),
            id_token: Some("id-sentinel".to_string()),
            expires_at: 1_800_000_000,
            account_id: "account-sentinel".to_string(),
            email: Some("person@example.com".to_string()),
        }
    }

    #[test]
    fn codex_oauth_roundtrips_but_redacted_settings_omit_it() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai-codex"))
            .expect("codex profile")
            .codex_oauth = Some(codex_credentials());

        let encoded = serde_json::to_string(&settings).expect("serialize settings");
        let decoded: AppSettings = serde_json::from_str(&encoded).expect("deserialize settings");
        assert_eq!(
            decoded
                .providers
                .get(&ProviderId::from("openai-codex"))
                .and_then(|profile| profile.codex_oauth.as_ref()),
            Some(&codex_credentials())
        );

        let redacted = serde_json::to_string(&settings.redacted()).expect("serialize redacted");
        for secret in [
            "access-sentinel",
            "refresh-sentinel",
            "id-sentinel",
            "account-sentinel",
        ] {
            assert!(
                !redacted.contains(secret),
                "redacted settings leaked {secret}"
            );
        }
        assert!(settings
            .redacted()
            .providers
            .get(&ProviderId::from("openai-codex"))
            .expect("codex profile")
            .codex_oauth
            .is_none());
    }

    #[test]
    fn merge_preserved_secrets_restores_codex_oauth() {
        let mut existing = AppSettings::default();
        existing
            .providers
            .get_mut(&ProviderId::from("openai-codex"))
            .expect("codex profile")
            .codex_oauth = Some(codex_credentials());
        let mut incoming = existing.redacted();

        merge_preserved_secrets(&mut incoming, &existing);

        assert_eq!(
            incoming
                .providers
                .get(&ProviderId::from("openai-codex"))
                .and_then(|profile| profile.codex_oauth.as_ref()),
            Some(&codex_credentials())
        );
    }

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
    fn provider_profiles_expose_fast_mode_separately_from_reasoning() {
        let settings = AppSettings::default();

        assert!(settings
            .providers
            .get(&ProviderId::from("openai-codex"))
            .is_some_and(|profile| profile.fast_mode_available));
        assert!(settings
            .providers
            .get(&ProviderId::from("openai"))
            .is_some_and(|profile| profile.fast_mode_available));
        assert!(!settings
            .providers
            .get(&ProviderId::from("anthropic"))
            .is_some_and(|profile| profile.fast_mode_available));
    }

    #[test]
    fn normalized_profile_migrates_legacy_fast_effort_label_to_none() {
        let mut settings = AppSettings::default();
        let profile = settings
            .providers
            .get_mut(&ProviderId::from("openai-codex"))
            .expect("Codex profile");
        let none = profile
            .reasoning_effort_options
            .iter_mut()
            .find(|option| option.value == "none")
            .expect("none effort");
        none.label = "Fast".to_string();

        let normalized = settings.normalized();
        let none = normalized
            .providers
            .get(&ProviderId::from("openai-codex"))
            .and_then(|profile| {
                profile
                    .reasoning_effort_options
                    .iter()
                    .find(|option| option.value == "none")
            })
            .expect("normalized none effort");

        assert_eq!(none.label, "None");
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
        assert!(profile.model_transports.is_empty());
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
    fn provider_profile_roundtrips_model_transport_overrides() {
        let mut profile = ProviderProfile::compatible_default();
        profile.model_transports.insert(
            "vendor-model".to_string(),
            ModelTransport::AnthropicMessages,
        );

        let value = serde_json::to_value(&profile).unwrap();
        assert_eq!(
            value["model_transports"]["vendor-model"],
            "anthropic_messages"
        );

        let restored: ProviderProfile = serde_json::from_value(value).unwrap();
        assert_eq!(
            restored.model_transports.get("vendor-model"),
            Some(&ModelTransport::AnthropicMessages)
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
    fn provider_profile_migrates_legacy_codex_display_name() {
        let spec = provider_spec(&ProviderId::from("openai-codex")).unwrap();
        let mut profile = ProviderProfile::from_spec(spec);
        profile.display_name = "OpenAI Codex".to_string();

        profile.normalize(Some(spec));

        assert_eq!(profile.display_name, "ChatGPT (Codex)");
    }

    #[test]
    fn mcp_settings_default_disables_external_discovery() {
        assert!(!McpSettings::default().discover_external);
    }

    #[test]
    fn app_settings_missing_mcp_key_disables_external_discovery() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("mcp");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert!(!parsed.mcp.discover_external);
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
                registry_base_url: McpSettings::default().registry_base_url,
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
                servers: vec![mcp_stdio_server(
                    "github",
                    "GitHub",
                    "npx",
                    vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
                    BTreeMap::new(),
                    true,
                )],
                ..McpSettings::default()
            },
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mcp.servers[0].id, "github");
    }

    #[test]
    fn normalized_mcp_legacy_config_migrates_losslessly_but_requires_new_trust() {
        let mut json = serde_json::to_value(AppSettings::default()).unwrap();
        json["mcp"] = serde_json::json!({
            "servers": [{
                "id": "massive",
                "displayName": "Massive MCP",
                "command": "npx",
                "args": ["--yes", "@acme/massive@1.2.3"],
                "env": {
                    "MASSIVE_API_KEY": "legacy-secret",
                    "MCP_LOG_LEVEL": "info"
                },
                "enabled": true
            }],
            "discoverExternal": true,
            "disabledDiscoveredIds": []
        });

        let parsed: AppSettings = serde_json::from_value(json).unwrap();
        let server = &parsed.mcp.servers[0];

        assert_eq!(
            server.schema_version,
            crate::mcp::model::MCP_SERVER_RECORD_VERSION
        );
        assert_eq!(server.source, crate::mcp::model::McpServerSource::Manual);
        assert_eq!(server.install, crate::mcp::model::McpInstall::External);
        let crate::mcp::model::McpConnection::Stdio {
            command,
            args,
            environment,
        } = &server.connection
        else {
            panic!("legacy command must migrate to stdio");
        };
        assert_eq!(command, "npx");
        assert_eq!(args, &["--yes", "@acme/massive@1.2.3"]);
        assert_eq!(
            environment["MASSIVE_API_KEY"],
            crate::mcp::model::PersistedValue::Literal {
                value: "legacy-secret".to_string()
            }
        );
        assert!(
            !server.enabled,
            "legacy executable needs explicit re-approval"
        );
        assert!(!crate::mcp::trust::is_trusted(server));
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
    fn redacted_clears_mcp_env_values_but_keeps_keys() {
        let mut settings = AppSettings::default();
        settings.mcp.servers.push(mcp_stdio_server(
            "massive",
            "Massive",
            "mcp_massive",
            vec![],
            BTreeMap::from([
                ("MASSIVE_API_KEY".into(), "secret".into()),
                ("MCP_TRANSPORT".into(), "stdio".into()),
            ]),
            true,
        ));

        let redacted = settings.redacted();

        assert_eq!(
            match &redacted.mcp.servers[0].connection {
                McpConnection::Stdio { environment, .. } => environment.get("MASSIVE_API_KEY"),
                _ => None,
            },
            Some(&PersistedValue::Literal {
                value: String::new()
            })
        );
        assert_eq!(
            match &redacted.mcp.servers[0].connection {
                McpConnection::Stdio { environment, .. } => environment.get("MCP_TRANSPORT"),
                _ => None,
            },
            Some(&PersistedValue::Literal {
                value: String::new()
            })
        );
    }

    #[test]
    fn merge_preserved_secrets_restores_search_keys() {
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

        merge_preserved_secrets(&mut incoming, &existing);
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
    fn merge_preserved_secrets_restores_redacted_mcp_env_values() {
        let server = |value: &str| {
            mcp_stdio_server(
                "massive",
                "Massive",
                "mcp_massive",
                vec![],
                BTreeMap::from([("MASSIVE_API_KEY".into(), value.into())]),
                true,
            )
        };
        let mut existing = AppSettings::default();
        existing.mcp.servers.push(server("secret"));
        let mut incoming = AppSettings::default();
        incoming.mcp.servers.push(server(""));

        merge_preserved_secrets(&mut incoming, &existing);

        assert_eq!(
            match &incoming.mcp.servers[0].connection {
                McpConnection::Stdio { environment, .. } => environment.get("MASSIVE_API_KEY"),
                _ => None,
            },
            Some(&PersistedValue::Literal {
                value: "secret".to_string()
            })
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
