/// Formats an AWS SDK error by walking the full `source` chain.
pub fn format_aws_sdk_error(error: &dyn std::error::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut current = error.source();
    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }
    parts.join(": ")
}

pub fn humanize_bedrock_sdk_error(message: &str) -> String {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("dispatch failure") {
        if lowered.contains("credentials")
            || lowered.contains("session token")
            || lowered.contains("not found or invalid")
            || lowered.contains("unable to locate")
        {
            return format!(
                "AWS credentials missing or expired. If you use SSO, enter your AWS profile name in Settings (the AWS CLI must be installed); OpenFlow will run `aws sso login` automatically when credentials expire. You can also run `aws sso login --profile <name>` in a terminal first and verify with `aws sts get-caller-identity --profile <name>`. For access keys, run `aws configure`. Raw AWS SDK error: {message}"
            );
        }
        return format!(
            "Could not reach Amazon Bedrock. Raw AWS SDK error: {message}. Check AWS region in Settings, network/VPN, proxy/TLS settings, and credentials (SSO: `aws sso login --profile <name>`; access keys: `aws configure`)."
        );
    }
    if lowered.contains("credentialsnotloaded")
        || lowered.contains("unable to load credentials")
        || lowered.contains("no credentials")
    {
        return "AWS credentials not configured. If you use SSO, enter your AWS profile name in Settings (the AWS CLI must be installed); OpenFlow will run `aws sso login` automatically when credentials expire. For access keys, run `aws configure`."
            .to_string();
    }
    if lowered.contains("model identifier is invalid") {
        return format!(
            "{message} Check the default model in Settings matches a Bedrock model ID exactly (for example `amazon.nova-pro-v1:0`)."
        );
    }
    message.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_aws_sdk_error_unwraps_source_chain() {
        #[derive(Debug)]
        struct LeafError(&'static str);
        impl std::fmt::Display for LeafError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for LeafError {}

        #[derive(Debug)]
        struct ConnectorError {
            source: LeafError,
        }
        impl std::fmt::Display for ConnectorError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "other")
            }
        }
        impl std::error::Error for ConnectorError {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.source)
            }
        }

        #[derive(Debug)]
        struct DispatchFailure {
            source: ConnectorError,
        }
        impl std::fmt::Display for DispatchFailure {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "dispatch failure")
            }
        }
        impl std::error::Error for DispatchFailure {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.source)
            }
        }

        let error = DispatchFailure {
            source: ConnectorError {
                source: LeafError("unable to locate credentials"),
            },
        };
        assert_eq!(
            format_aws_sdk_error(&error),
            "dispatch failure: other: unable to locate credentials"
        );

        let message = humanize_bedrock_sdk_error(&format_aws_sdk_error(&error));
        assert!(message.contains("unable to locate credentials"));
        assert!(message.contains("aws sso login"));
    }

    #[test]
    fn humanize_bedrock_sdk_error_mentions_sso_for_credential_failures() {
        let message = humanize_bedrock_sdk_error(
            "dispatch failure: credentials provider failed: unable to locate credentials",
        );
        assert!(message.contains("aws sso login"));
        assert!(message.contains("AWS profile name in Settings"));
        assert!(message.contains("Raw AWS SDK error"));
        assert!(message.contains("unable to locate credentials"));

        let message = humanize_bedrock_sdk_error("CredentialsNotLoaded: no credentials configured");
        assert!(message.contains("aws sso login"));
    }

    #[test]
    fn humanize_bedrock_sdk_error_preserves_dispatch_failure_detail() {
        let message = humanize_bedrock_sdk_error(
            "dispatch failure: connector error: certificate verify failed for bedrock-runtime.ap-southeast-2.amazonaws.com",
        );

        assert!(message.contains("Could not reach Amazon Bedrock"));
        assert!(message.contains("Raw AWS SDK error"));
        assert!(message.contains("certificate verify failed"));
        assert!(message.contains("proxy/TLS settings"));
    }
}
