use crate::mcp::client_capabilities::{McpClientRequestKind, PendingMcpClientRequest};
use crate::mcp::model::McpPolicy;
use engine::NodeId;
use parking_lot::Mutex;
use reqwest::Url;
use rmcp::model::{
    ClientCapabilities, ClientInfo, ContextInclusion, CreateElicitationRequestParams,
    CreateElicitationResult, CreateMessageRequestParams, CreateMessageResult,
    ElicitationCapability, FormElicitationCapability, ListRootsResult, Root, SamplingCapability,
    UrlElicitationCapability,
};
use rmcp::service::{RequestContext, RoleClient};
use rmcp::{ClientHandler, ErrorData};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

const MAX_CALLBACK_MESSAGE_BYTES: usize = 32 * 1_024;
const MAX_CALLBACK_PAYLOAD_BYTES: usize = 256 * 1_024;
const MAX_SAMPLING_REQUESTS_PER_RUN: u32 = 64;
const MAX_SAMPLING_TOKENS_PER_REQUEST: u32 = 65_536;
const MAX_SAMPLING_TOTAL_TOKENS_PER_RUN: u32 = 262_144;
const MAX_ELICITATION_REQUESTS_PER_RUN: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRequestOrigin {
    pub node_id: NodeId,
    pub tool_call_id: String,
    pub tool_name: String,
}

#[derive(Debug)]
pub enum McpRunClientRequestPayload {
    Sampling(CreateMessageRequestParams),
    Elicitation(CreateElicitationRequestParams),
}

#[derive(Debug)]
pub enum McpRunClientResponse {
    Sampling(CreateMessageResult),
    Elicitation(CreateElicitationResult),
}

#[derive(Debug)]
pub struct McpRunClientRequest {
    pub pending: PendingMcpClientRequest,
    pub payload: McpRunClientRequestPayload,
    pub response_tx: oneshot::Sender<Result<McpRunClientResponse, ErrorData>>,
}

#[derive(Debug, Default)]
struct CallbackBudgetUsage {
    sampling_requests: u32,
    sampling_tokens: u32,
    elicitation_requests: u32,
}

#[derive(Clone)]
pub struct OpenFlowMcpClientHandler {
    server_id: String,
    policy: McpPolicy,
    roots: Arc<Vec<Root>>,
    callback_tx: Option<UnboundedSender<McpRunClientRequest>>,
    active_origin: Arc<Mutex<Option<McpRequestOrigin>>>,
    budget: Arc<Mutex<CallbackBudgetUsage>>,
}

impl std::fmt::Debug for OpenFlowMcpClientHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenFlowMcpClientHandler")
            .field("server_id", &self.server_id)
            .field("root_count", &self.roots.len())
            .field("callbacks_connected", &self.callback_tx.is_some())
            .finish_non_exhaustive()
    }
}

impl OpenFlowMcpClientHandler {
    pub fn disconnected(server_id: impl Into<String>, policy: McpPolicy) -> Self {
        Self {
            server_id: server_id.into(),
            policy,
            roots: Arc::new(Vec::new()),
            callback_tx: None,
            active_origin: Arc::new(Mutex::new(None)),
            budget: Arc::new(Mutex::new(CallbackBudgetUsage::default())),
        }
    }

    pub fn for_run(
        server_id: impl Into<String>,
        policy: McpPolicy,
        project_root: Option<&Path>,
        callback_tx: UnboundedSender<McpRunClientRequest>,
    ) -> Self {
        let roots = if policy.allow_roots {
            project_root.and_then(root_from_path).into_iter().collect()
        } else {
            Vec::new()
        };
        Self {
            server_id: server_id.into(),
            policy,
            roots: Arc::new(roots),
            callback_tx: Some(callback_tx),
            active_origin: Arc::new(Mutex::new(None)),
            budget: Arc::new(Mutex::new(CallbackBudgetUsage::default())),
        }
    }

    pub fn callbacks_enabled(&self) -> bool {
        self.callback_tx.is_some() && (self.policy.allow_sampling || self.policy.allow_elicitation)
    }

    pub fn set_active_origin(&self, origin: McpRequestOrigin) -> ActiveOriginGuard {
        *self.active_origin.lock() = Some(origin);
        ActiveOriginGuard {
            active_origin: Arc::clone(&self.active_origin),
        }
    }

    fn origin(&self) -> Result<McpRequestOrigin, ErrorData> {
        self.active_origin.lock().clone().ok_or_else(|| {
            ErrorData::invalid_request("MCP callback has no active tool request", None)
        })
    }

    fn reserve_sampling(&self, params: &CreateMessageRequestParams) -> Result<(), ErrorData> {
        if !self.policy.allow_sampling || self.callback_tx.is_none() {
            return Err(ErrorData::invalid_request("MCP sampling is disabled", None));
        }
        if params.task.is_some() || params.tools.is_some() || params.tool_choice.is_some() {
            return Err(ErrorData::invalid_params(
                "MCP sampling tasks and tools are disabled",
                None,
            ));
        }
        if !matches!(params.include_context, None | Some(ContextInclusion::None)) {
            return Err(ErrorData::invalid_params(
                "MCP sampling context inclusion is disabled",
                None,
            ));
        }
        let per_request_limit = self
            .policy
            .sampling_max_tokens_per_request
            .min(MAX_SAMPLING_TOKENS_PER_REQUEST);
        if params.max_tokens == 0 || params.max_tokens > per_request_limit {
            return Err(ErrorData::invalid_params(
                "MCP sampling token limit exceeds server policy",
                None,
            ));
        }
        let payload_bytes = serde_json::to_vec(params)
            .map_err(|_| ErrorData::invalid_params("MCP sampling payload is invalid", None))?
            .len();
        if payload_bytes > MAX_CALLBACK_PAYLOAD_BYTES {
            return Err(ErrorData::invalid_params(
                "MCP sampling payload exceeds client limit",
                None,
            ));
        }
        params
            .validate()
            .map_err(|_| ErrorData::invalid_params("MCP sampling messages are invalid", None))?;
        if params.messages.iter().any(|message| {
            message
                .content
                .iter()
                .any(|content| content.as_text().is_none())
        }) {
            return Err(ErrorData::invalid_params(
                "MCP sampling currently accepts text messages only",
                None,
            ));
        }
        let mut usage = self.budget.lock();
        let next_requests = usage.sampling_requests.saturating_add(1);
        let next_tokens = usage.sampling_tokens.saturating_add(params.max_tokens);
        if next_requests
            > self
                .policy
                .sampling_max_requests_per_run
                .min(MAX_SAMPLING_REQUESTS_PER_RUN)
            || next_tokens
                > self
                    .policy
                    .sampling_max_total_tokens_per_run
                    .min(MAX_SAMPLING_TOTAL_TOKENS_PER_RUN)
        {
            return Err(ErrorData::invalid_request(
                "MCP sampling run budget exhausted",
                None,
            ));
        }
        usage.sampling_requests = next_requests;
        usage.sampling_tokens = next_tokens;
        Ok(())
    }

    fn reserve_elicitation(
        &self,
        params: &CreateElicitationRequestParams,
    ) -> Result<(), ErrorData> {
        if !self.policy.allow_elicitation || self.callback_tx.is_none() {
            return Err(ErrorData::invalid_request(
                "MCP elicitation is disabled",
                None,
            ));
        }
        let (message, url) = match params {
            CreateElicitationRequestParams::FormElicitationParams { message, .. } => {
                (message.as_str(), None)
            }
            CreateElicitationRequestParams::UrlElicitationParams { message, url, .. } => {
                (message.as_str(), Some(url.as_str()))
            }
        };
        if message.is_empty() || message.len() > MAX_CALLBACK_MESSAGE_BYTES {
            return Err(ErrorData::invalid_params(
                "MCP elicitation message exceeds client limit",
                None,
            ));
        }
        let payload_bytes = serde_json::to_vec(params)
            .map_err(|_| ErrorData::invalid_params("MCP elicitation payload is invalid", None))?
            .len();
        if payload_bytes > MAX_CALLBACK_PAYLOAD_BYTES {
            return Err(ErrorData::invalid_params(
                "MCP elicitation payload exceeds client limit",
                None,
            ));
        }
        if let Some(url) = url {
            validate_elicitation_url(url)?;
        }
        let mut usage = self.budget.lock();
        let next = usage.elicitation_requests.saturating_add(1);
        if next
            > self
                .policy
                .elicitation_max_requests_per_run
                .min(MAX_ELICITATION_REQUESTS_PER_RUN)
        {
            return Err(ErrorData::invalid_request(
                "MCP elicitation run budget exhausted",
                None,
            ));
        }
        usage.elicitation_requests = next;
        Ok(())
    }

    async fn request_host(
        &self,
        pending: PendingMcpClientRequest,
        payload: McpRunClientRequestPayload,
        context: RequestContext<RoleClient>,
    ) -> Result<McpRunClientResponse, ErrorData> {
        let callback_tx = self.callback_tx.as_ref().ok_or_else(|| {
            ErrorData::invalid_request("MCP client callback host is unavailable", None)
        })?;
        let (response_tx, response_rx) = oneshot::channel();
        callback_tx
            .send(McpRunClientRequest {
                pending,
                payload,
                response_tx,
            })
            .map_err(|_| ErrorData::invalid_request("MCP run host is unavailable", None))?;
        tokio::select! {
            () = context.ct.cancelled() => Err(ErrorData::invalid_request("MCP callback was cancelled", None)),
            response = response_rx => response
                .map_err(|_| ErrorData::invalid_request("MCP callback response was cancelled", None))?,
        }
    }
}

pub struct ActiveOriginGuard {
    active_origin: Arc<Mutex<Option<McpRequestOrigin>>>,
}

impl Drop for ActiveOriginGuard {
    fn drop(&mut self) {
        self.active_origin.lock().take();
    }
}

impl ClientHandler for OpenFlowMcpClientHandler {
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ErrorData> {
        let origin = self.origin()?;
        self.reserve_sampling(&params)?;
        let pending = PendingMcpClientRequest {
            request_id: Uuid::new_v4().to_string(),
            server_id: self.server_id.clone(),
            node_id: origin.node_id,
            tool_call_id: origin.tool_call_id,
            tool_name: origin.tool_name,
            kind: McpClientRequestKind::Sampling,
            message: "Server requests an approved model sampling call.".to_string(),
            requested_schema: None,
            url: None,
            max_tokens: Some(params.max_tokens),
        };
        match self
            .request_host(
                pending,
                McpRunClientRequestPayload::Sampling(params),
                context,
            )
            .await?
        {
            McpRunClientResponse::Sampling(result) => Ok(result),
            McpRunClientResponse::Elicitation(_) => Err(ErrorData::invalid_request(
                "MCP callback host returned the wrong response type",
                None,
            )),
        }
    }

    async fn list_roots(
        &self,
        _context: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, ErrorData> {
        Ok(ListRootsResult::new(self.roots.as_ref().clone()))
    }

    async fn create_elicitation(
        &self,
        params: CreateElicitationRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        let origin = self.origin()?;
        self.reserve_elicitation(&params)?;
        let (kind, message, requested_schema, url) = match &params {
            CreateElicitationRequestParams::FormElicitationParams {
                message,
                requested_schema,
                ..
            } => (
                McpClientRequestKind::ElicitationForm,
                message.clone(),
                serde_json::to_value(requested_schema).ok(),
                None,
            ),
            CreateElicitationRequestParams::UrlElicitationParams { message, url, .. } => (
                McpClientRequestKind::ElicitationUrl,
                message.clone(),
                None,
                Some(url.clone()),
            ),
        };
        let pending = PendingMcpClientRequest {
            request_id: Uuid::new_v4().to_string(),
            server_id: self.server_id.clone(),
            node_id: origin.node_id,
            tool_call_id: origin.tool_call_id,
            tool_name: origin.tool_name,
            kind,
            message,
            requested_schema,
            url,
            max_tokens: None,
        };
        match self
            .request_host(
                pending,
                McpRunClientRequestPayload::Elicitation(params),
                context,
            )
            .await?
        {
            McpRunClientResponse::Elicitation(result) => Ok(result),
            McpRunClientResponse::Sampling(_) => Err(ErrorData::invalid_request(
                "MCP callback host returned the wrong response type",
                None,
            )),
        }
    }

    fn get_info(&self) -> ClientInfo {
        let mut info = ClientInfo::default();
        let mut capabilities = ClientCapabilities::default();
        if self.policy.allow_roots && !self.roots.is_empty() {
            capabilities.roots = Some(Default::default());
        }
        if self.callback_tx.is_some() && self.policy.allow_sampling {
            capabilities.sampling = Some(SamplingCapability::default());
        }
        if self.callback_tx.is_some() && self.policy.allow_elicitation {
            capabilities.elicitation = Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: Some(true),
                }),
                url: Some(UrlElicitationCapability {}),
            });
        }
        info.capabilities = capabilities;
        info
    }
}

fn root_from_path(path: &Path) -> Option<Root> {
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    let uri = Url::from_directory_path(&canonical).ok()?.to_string();
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Project")
        .to_string();
    Some(Root::new(uri).with_name(name))
}

fn validate_elicitation_url(value: &str) -> Result<(), ErrorData> {
    let url = Url::parse(value)
        .map_err(|_| ErrorData::invalid_params("MCP elicitation URL is invalid", None))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ErrorData::invalid_params(
            "MCP elicitation URL must be credential-free HTTPS without a fragment",
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Role, SamplingMessage, SamplingMessageContent};
    use std::path::PathBuf;

    fn sampling(max_tokens: u32) -> CreateMessageRequestParams {
        CreateMessageRequestParams::new(
            vec![SamplingMessage::new(
                Role::User,
                SamplingMessageContent::text("hello"),
            )],
            max_tokens,
        )
    }

    #[test]
    fn client_capabilities_default_deny() {
        let handler = OpenFlowMcpClientHandler::disconnected("server", McpPolicy::default());
        let info = handler.get_info();
        assert!(info.capabilities.roots.is_none());
        assert!(info.capabilities.sampling.is_none());
        assert!(info.capabilities.elicitation.is_none());
        assert!(handler.reserve_sampling(&sampling(1)).is_err());
    }

    #[test]
    fn sampling_budget_rejects_tools_and_exhaustion() {
        let mut policy = McpPolicy {
            allow_sampling: true,
            sampling_max_requests_per_run: 1,
            sampling_max_tokens_per_request: 8,
            sampling_max_total_tokens_per_run: 8,
            ..McpPolicy::default()
        };
        let (callback_tx, _callback_rx) = tokio::sync::mpsc::unbounded_channel();
        let handler =
            OpenFlowMcpClientHandler::for_run("server", policy.clone(), None, callback_tx);
        assert!(handler.reserve_sampling(&sampling(8)).is_ok());
        assert!(handler.reserve_sampling(&sampling(1)).is_err());

        policy.sampling_max_requests_per_run = 2;
        let (callback_tx, _callback_rx) = tokio::sync::mpsc::unbounded_channel();
        let handler = OpenFlowMcpClientHandler::for_run("server", policy, None, callback_tx);
        let mut with_tools = sampling(1);
        with_tools.tools = Some(Vec::new());
        assert!(handler.reserve_sampling(&with_tools).is_err());

        let policy = McpPolicy {
            allow_sampling: true,
            sampling_max_requests_per_run: u32::MAX,
            sampling_max_tokens_per_request: u32::MAX,
            sampling_max_total_tokens_per_run: u32::MAX,
            ..McpPolicy::default()
        };
        let (callback_tx, _callback_rx) = tokio::sync::mpsc::unbounded_channel();
        let handler = OpenFlowMcpClientHandler::for_run("server", policy, None, callback_tx);
        assert!(handler
            .reserve_sampling(&sampling(MAX_SAMPLING_TOKENS_PER_REQUEST + 1))
            .is_err());
    }

    #[test]
    fn root_is_only_exposed_for_allowed_existing_project_dir() {
        let mut policy = McpPolicy {
            allow_roots: true,
            ..McpPolicy::default()
        };
        let (callback_tx, _callback_rx) = tokio::sync::mpsc::unbounded_channel();
        let handler = OpenFlowMcpClientHandler::for_run(
            "server",
            policy.clone(),
            Some(PathBuf::from(".").as_path()),
            callback_tx,
        );
        assert_eq!(
            handler.get_info().capabilities.roots,
            Some(Default::default())
        );
        assert_eq!(handler.roots.len(), 1);

        policy.allow_roots = false;
        let handler = OpenFlowMcpClientHandler::disconnected("server", policy);
        assert!(handler.get_info().capabilities.roots.is_none());
    }

    #[test]
    fn elicitation_url_requires_safe_https() {
        assert!(validate_elicitation_url("https://example.com/consent").is_ok());
        assert!(validate_elicitation_url("http://example.com/consent").is_err());
        assert!(validate_elicitation_url("https://user:pass@example.com/consent").is_err());
        assert!(validate_elicitation_url("https://example.com/consent#secret").is_err());
    }
}
