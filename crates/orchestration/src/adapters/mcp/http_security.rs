use reqwest::{redirect::Policy, Client, Url};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SecuredHttpEndpoint {
    pub url: Url,
    pub client: Client,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpSecurityError {
    #[error("remote MCP endpoint URL is invalid")]
    InvalidUrl,
    #[error("remote MCP endpoint must not contain user credentials or a fragment")]
    EmbeddedCredentialsOrFragment,
    #[error("remote MCP endpoints require HTTPS")]
    HttpsRequired,
    #[error("localhost MCP endpoints require explicit local development approval")]
    LocalhostApprovalRequired,
    #[error("remote MCP endpoint resolves to a blocked network address")]
    BlockedAddress,
    #[error("remote MCP endpoint DNS resolution failed")]
    DnsResolutionFailed,
    #[error("remote MCP HTTP client setup failed")]
    ClientBuildFailed,
}

pub fn validate_endpoint_url(
    endpoint: &str,
    allow_localhost: bool,
) -> Result<Url, HttpSecurityError> {
    let url = Url::parse(endpoint).map_err(|_| HttpSecurityError::InvalidUrl)?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(HttpSecurityError::EmbeddedCredentialsOrFragment);
    }
    let host = url.host_str().ok_or(HttpSecurityError::InvalidUrl)?;
    let local_host = is_localhost_name(host)
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    match url.scheme() {
        "https" => {}
        "http" if local_host && allow_localhost => {}
        "http" if local_host => return Err(HttpSecurityError::LocalhostApprovalRequired),
        _ => return Err(HttpSecurityError::HttpsRequired),
    }
    if local_host && !allow_localhost {
        return Err(HttpSecurityError::LocalhostApprovalRequired);
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        validate_address(address, allow_localhost)?;
    }
    Ok(url)
}

pub async fn secure_http_endpoint(
    endpoint: &str,
    allow_localhost: bool,
) -> Result<SecuredHttpEndpoint, HttpSecurityError> {
    let url = validate_endpoint_url(endpoint, allow_localhost)?;
    let host = url
        .host_str()
        .ok_or(HttpSecurityError::InvalidUrl)?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or(HttpSecurityError::InvalidUrl)?;
    let addresses = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| HttpSecurityError::DnsResolutionFailed)?
        .collect::<BTreeSet<SocketAddr>>();
    if addresses.is_empty() {
        return Err(HttpSecurityError::DnsResolutionFailed);
    }
    for address in &addresses {
        validate_address(address.ip(), allow_localhost)?;
    }
    let addresses = addresses.into_iter().collect::<Vec<_>>();
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(Policy::none())
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|_| HttpSecurityError::ClientBuildFailed)?;
    Ok(SecuredHttpEndpoint { url, client })
}

fn validate_address(address: IpAddr, allow_localhost: bool) -> Result<(), HttpSecurityError> {
    if address.is_loopback() {
        return if allow_localhost {
            Ok(())
        } else {
            Err(HttpSecurityError::LocalhostApprovalRequired)
        };
    }
    let blocked = match address {
        IpAddr::V4(address) => blocked_ipv4(address),
        IpAddr::V6(address) => blocked_ipv6(address),
    };
    if blocked {
        Err(HttpSecurityError::BlockedAddress)
    } else {
        Ok(())
    }
}

fn blocked_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, _, _] = address.octets();
    address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_unspecified()
        || address.is_broadcast()
        || address.is_documentation()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0)
        || (a == 198 && matches!(b, 18 | 19))
}

fn blocked_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    address.is_unspecified()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || address.to_ipv4_mapped().is_some_and(blocked_ipv4)
}

fn is_localhost_name(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_security_rejects_credentials_fragments_and_non_https_remote_urls() {
        assert_eq!(
            validate_endpoint_url("https://user:pass@example.com/mcp", false),
            Err(HttpSecurityError::EmbeddedCredentialsOrFragment)
        );
        assert_eq!(
            validate_endpoint_url("https://example.com/mcp#token", false),
            Err(HttpSecurityError::EmbeddedCredentialsOrFragment)
        );
        assert_eq!(
            validate_endpoint_url("http://example.com/mcp", false),
            Err(HttpSecurityError::HttpsRequired)
        );
    }

    #[test]
    fn http_security_blocks_private_and_requires_localhost_opt_in() {
        assert_eq!(
            validate_endpoint_url("https://10.0.0.1/mcp", false),
            Err(HttpSecurityError::BlockedAddress)
        );
        assert_eq!(
            validate_endpoint_url("http://127.0.0.1:3000/mcp", false),
            Err(HttpSecurityError::LocalhostApprovalRequired)
        );
        assert!(validate_endpoint_url("http://127.0.0.1:3000/mcp", true).is_ok());
        assert!(validate_endpoint_url("https://example.com/mcp", false).is_ok());
    }

    #[tokio::test]
    async fn secured_client_pins_explicitly_approved_loopback_and_rejects_redirects() {
        let secured = secure_http_endpoint("http://127.0.0.1:9/mcp", true)
            .await
            .unwrap();
        assert_eq!(secured.url.scheme(), "http");
    }
}
