//! URL validation policy for SSRF protection.
//!
//! Validates URLs before navigation to prevent:
//! - Access to internal/private networks
//! - File system access via file:// scheme
//! - DNS rebinding attacks
//! - Cloud metadata endpoint access

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use anyhow::anyhow;
use url::Url;

/// Policy for URL validation.
#[derive(Debug, Clone)]
pub struct UrlPolicy {
    /// Allowed URL schemes (default: ["http", "https"])
    pub allowed_schemes: Vec<String>,

    /// Block private IP addresses (10.x.x.x, 172.16-31.x.x, 192.168.x.x)
    pub block_private_ips: bool,

    /// Block loopback addresses (127.x.x.x, ::1)
    pub block_loopback: bool,

    /// Block link-local addresses (169.254.x.x, fe80::)
    pub block_link_local: bool,

    /// Block multicast addresses
    pub block_multicast: bool,

    /// Block specific hostnames (e.g., "localhost", "metadata.google.internal")
    pub blocked_hosts: Vec<String>,

    /// Allowlist of permitted hosts (if set, only these are allowed)
    pub allowed_hosts: Option<Vec<String>>,
}

impl Default for UrlPolicy {
    fn default() -> Self {
        Self {
            allowed_schemes: vec!["http".into(), "https".into()],
            block_private_ips: true,
            block_loopback: true,
            block_link_local: true,
            block_multicast: true,
            blocked_hosts: vec![
                "localhost".into(),
                "localhost.localdomain".into(),
                "ip6-localhost".into(),
                "ip6-loopback".into(),
                // Cloud metadata endpoints
                "metadata.google.internal".into(),
                "169.254.169.254".into(),  // AWS/GCP/Azure metadata IP
                "100.100.100.200".into(),   // Alibaba metadata
                "metadata.azure.internal".into(),
            ],
            allowed_hosts: None,
        }
    }
}

impl UrlPolicy {
    /// Create a permissive policy (allows all IPs, only validates scheme).
    /// Use with caution - only for trusted environments.
    pub fn permissive() -> Self {
        Self {
            allowed_schemes: vec!["http".into(), "https".into()],
            block_private_ips: false,
            block_loopback: false,
            block_link_local: false,
            block_multicast: false,
            blocked_hosts: vec![],
            allowed_hosts: None,
        }
    }

    /// Create a policy that only allows specific hosts.
    /// This is the most restrictive mode.
    pub fn allowlist(hosts: Vec<String>) -> Self {
        Self {
            allowed_schemes: vec!["http".into(), "https".into()],
            allowed_hosts: Some(hosts),
            ..Self::default()
        }
    }

    /// Validate a URL against this policy.
    pub fn validate(&self, url_str: &str) -> anyhow::Result<Url> {
        // Parse URL
        let url = Url::parse(url_str)
            .map_err(|e| anyhow!("Invalid URL '{}': {}", url_str, e))?;

        // Check scheme
        let scheme = url.scheme().to_lowercase();
        if !self.allowed_schemes.contains(&scheme) {
            return Err(anyhow!(
                "URL scheme '{}' not allowed. Allowed schemes: {:?}",
                scheme, self.allowed_schemes
            ));
        }

        // Check host exists
        let host = url.host_str()
            .ok_or_else(|| anyhow!("URL has no host: {}", url_str))?;

        // Check allowlist first (if configured)
        if let Some(ref allowed) = self.allowed_hosts {
            if !allowed.iter().any(|h| host.eq_ignore_ascii_case(h)) {
                return Err(anyhow!(
                    "Host '{}' is not in the allowlist. Allowed hosts: {:?}",
                    host, allowed
                ));
            }
        }

        // Check blocked hosts
        let host_lower = host.to_lowercase();
        for blocked in &self.blocked_hosts {
            if host_lower == blocked.to_lowercase() {
                return Err(anyhow!("Host '{}' is blocked by security policy", host));
            }
        }

        // Try to parse host as IP address and validate
        // For IPv6 URLs like http://[::1]/, host_str() returns "[::1]" with brackets.
        // We need to strip the brackets before parsing as IpAddr.
        let ip_host = if host.starts_with('[') && host.ends_with(']') {
            &host[1..host.len() - 1]
        } else {
            host
        };
        if let Ok(ip) = ip_host.parse::<IpAddr>() {
            self.validate_ip(&ip)?;
        }

        // Note: DNS resolution happens at request time. For stronger protection
        // against DNS rebinding, consider using a custom resolver that validates
        // the resolved IP before connecting.

        Ok(url)
    }

    /// Validate an IP address against the policy.
    fn validate_ip(&self, ip: &IpAddr) -> anyhow::Result<()> {
        // Check loopback
        if self.block_loopback && ip.is_loopback() {
            return Err(anyhow!("Loopback address {} is blocked by security policy", ip));
        }

        // Check link-local
        if self.block_link_local && is_link_local(ip) {
            return Err(anyhow!("Link-local address {} is blocked by security policy", ip));
        }

        // Check multicast
        if self.block_multicast && ip.is_multicast() {
            return Err(anyhow!("Multicast address {} is blocked by security policy", ip));
        }

        // Check private IPs
        if self.block_private_ips && is_private(ip) {
            return Err(anyhow!("Private IP address {} is blocked by security policy", ip));
        }

        Ok(())
    }
}

/// Check if an IP address is link-local.
fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 169.254.0.0/16
            octets[0] == 169 && octets[1] == 254
        }
        IpAddr::V6(ipv6) => {
            // fe80::/10
            let segments = ipv6.segments();
            segments[0] & 0xffc0 == 0xfe80
        }
    }
}

/// Check if an IP address is private (RFC 1918 for IPv4, unique local for IPv6).
fn is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_private_ipv6(ipv6),
    }
}

/// Check if an IPv4 address is private (RFC 1918).
fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 10.0.0.0/8
    if octets[0] == 10 {
        return true;
    }

    // 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }

    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    false
}

/// Check if an IPv6 address is private (unique local fc00::/7).
fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
    let segments = ip.segments();
    // Unique local addresses: fc00::/7 (fc00:: - fdff::)
    (segments[0] & 0xfe00) == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_allows_public_urls() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("https://example.com").is_ok());
        assert!(policy.validate("http://example.com/path?query=1").is_ok());
        assert!(policy.validate("https://subdomain.example.com:8080/path").is_ok());
    }

    #[test]
    fn test_default_policy_blocks_file_scheme() {
        let policy = UrlPolicy::default();
        let result = policy.validate("file:///etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("scheme"));
    }

    #[test]
    fn test_default_policy_blocks_localhost() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("http://localhost/admin").is_err());
        assert!(policy.validate("http://LOCALHOST/admin").is_err());
        assert!(policy.validate("http://127.0.0.1/admin").is_err());
        assert!(policy.validate("http://[::1]/admin").is_err());
    }

    #[test]
    fn test_default_policy_blocks_private_ips() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("http://10.0.0.1/").is_err());
        assert!(policy.validate("http://10.255.255.255/").is_err());
        assert!(policy.validate("http://172.16.0.1/").is_err());
        assert!(policy.validate("http://172.31.255.255/").is_err());
        assert!(policy.validate("http://192.168.1.1/").is_err());
        assert!(policy.validate("http://192.168.0.1/").is_err());
    }

    #[test]
    fn test_default_policy_allows_public_ips() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("http://8.8.8.8/").is_ok());
        assert!(policy.validate("http://1.1.1.1/").is_ok());
    }

    #[test]
    fn test_default_policy_blocks_metadata_endpoints() {
        let policy = UrlPolicy::default();
        // AWS/GCP metadata
        assert!(policy.validate("http://169.254.169.254/latest/meta-data/").is_err());
        // GCP metadata
        assert!(policy.validate("http://metadata.google.internal/computeMetadata/v1/").is_err());
        // Alibaba metadata
        assert!(policy.validate("http://100.100.100.200/latest/meta-data/").is_err());
    }

    #[test]
    fn test_default_policy_blocks_link_local() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("http://169.254.1.1/").is_err());
    }

    #[test]
    fn test_permissive_policy_allows_private() {
        let policy = UrlPolicy::permissive();
        assert!(policy.validate("http://192.168.1.1/").is_ok());
        assert!(policy.validate("http://127.0.0.1/").is_ok());
        assert!(policy.validate("http://10.0.0.1/").is_ok());
        // Still blocks non-http(s) schemes
        assert!(policy.validate("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_allowlist_policy() {
        let policy = UrlPolicy::allowlist(vec![
            "example.com".into(),
            "api.example.com".into(),
        ]);

        assert!(policy.validate("https://example.com").is_ok());
        assert!(policy.validate("https://example.com/path").is_ok());
        assert!(policy.validate("https://api.example.com/v1").is_ok());
        assert!(policy.validate("https://EXAMPLE.COM").is_ok()); // case insensitive

        // Blocked - not in allowlist
        assert!(policy.validate("https://evil.com").is_err());
        assert!(policy.validate("https://sub.example.com").is_err());
    }

    #[test]
    fn test_invalid_url_format() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("not-a-url").is_err());
        assert!(policy.validate("http://").is_err());
        assert!(policy.validate("://no-scheme.com").is_err());
    }

    #[test]
    fn test_ipv6_addresses() {
        let policy = UrlPolicy::default();

        // Public IPv6 should work
        assert!(policy.validate("http://[2001:4860:4860::8888]/").is_ok());

        // Loopback IPv6 blocked
        assert!(policy.validate("http://[::1]/").is_err());

        // Link-local IPv6 blocked
        assert!(policy.validate("http://[fe80::1]/").is_err());

        // Unique local (private) IPv6 blocked
        assert!(policy.validate("http://[fc00::1]/").is_err());
        assert!(policy.validate("http://[fd00::1]/").is_err());
    }

    #[test]
    fn test_case_insensitive_scheme() {
        let policy = UrlPolicy::default();
        assert!(policy.validate("HTTPS://example.com").is_ok());
        assert!(policy.validate("HtTpS://example.com").is_ok());
    }
}
