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
    AwsCredentials {
        profile: Option<String>,
        region: String,
    },
}

impl AuthConfig {
    #[must_use]
    pub const fn requires_key(&self) -> bool {
        match self {
            Self::Bearer { required, .. } | Self::Header { required, .. } => *required,
            Self::NoneAllowed | Self::AwsCredentials { .. } => false,
        }
    }
}
