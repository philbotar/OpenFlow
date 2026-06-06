use reqwest::RequestBuilder;
use workflow_core::AgentError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    Bearer {
        api_key: Option<String>,
        required: bool,
    },
    Header {
        name: String,
        api_key: Option<String>,
        required: bool,
    },
    NoneAllowed,
}

impl AuthConfig {
    #[must_use]
    pub fn has_key(&self) -> bool {
        match self {
            Self::Bearer { api_key, .. } | Self::Header { api_key, .. } => api_key
                .as_deref()
                .map(str::trim)
                .is_some_and(|key| !key.is_empty()),
            Self::NoneAllowed => true,
        }
    }

    #[must_use]
    pub const fn requires_key(&self) -> bool {
        match self {
            Self::Bearer { required, .. } | Self::Header { required, .. } => *required,
            Self::NoneAllowed => false,
        }
    }
}

pub fn apply_auth(
    request: RequestBuilder,
    auth: &AuthConfig,
    label: &str,
) -> Result<RequestBuilder, AgentError> {
    match auth {
        AuthConfig::Bearer { api_key, required } => {
            apply_bearer_auth(request, api_key.as_ref(), *required, label)
        }
        AuthConfig::Header {
            name,
            api_key,
            required,
        } => apply_header_auth(request, name, api_key.as_ref(), *required, label),
        AuthConfig::NoneAllowed => Ok(request),
    }
}

fn apply_bearer_auth(
    request: RequestBuilder,
    api_key: Option<&String>,
    required: bool,
    label: &str,
) -> Result<RequestBuilder, AgentError> {
    let Some(api_key) = api_key
        .map(String::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
    else {
        return if required {
            Err(AgentError::Failed(format!("{label} API key missing")))
        } else {
            Ok(request)
        };
    };
    Ok(request.bearer_auth(api_key))
}

fn apply_header_auth(
    request: RequestBuilder,
    name: &str,
    api_key: Option<&String>,
    required: bool,
    label: &str,
) -> Result<RequestBuilder, AgentError> {
    let Some(api_key) = api_key
        .map(String::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
    else {
        return if required {
            Err(AgentError::Failed(format!("{label} API key missing")))
        } else {
            Ok(request)
        };
    };
    Ok(request.header(name, api_key))
}
