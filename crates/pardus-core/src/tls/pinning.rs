use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl Default for PinAlgorithm {
    fn default() -> Self {
        Self::Sha256
    }
}

impl PinAlgorithm {
    pub fn hash_length(&self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    pub fn digest(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha256 => {
                use sha2::Digest as _;
                sha2::Sha256::digest(data).to_vec()
            }
            Self::Sha384 => {
                use sha2::Digest as _;
                sha2::Sha384::digest(data).to_vec()
            }
            Self::Sha512 => {
                use sha2::Digest as _;
                sha2::Sha512::digest(data).to_vec()
            }
        }
    }
}

impl fmt::Display for PinAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256 => write!(f, "sha256"),
            Self::Sha384 => write!(f, "sha384"),
            Self::Sha512 => write!(f, "sha512"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CertPin {
    SpkiHash {
        algorithm: PinAlgorithm,
        hash: String,
    },
    CaCertificate {
        der_base64: String,
        subject: Option<String>,
    },
}

impl CertPin {
    pub fn spki_sha256(base64_hash: impl Into<String>) -> Self {
        Self::SpkiHash {
            algorithm: PinAlgorithm::Sha256,
            hash: base64_hash.into(),
        }
    }

    pub fn spki_sha384(base64_hash: impl Into<String>) -> Self {
        Self::SpkiHash {
            algorithm: PinAlgorithm::Sha384,
            hash: base64_hash.into(),
        }
    }

    pub fn spki_sha512(base64_hash: impl Into<String>) -> Self {
        Self::SpkiHash {
            algorithm: PinAlgorithm::Sha512,
            hash: base64_hash.into(),
        }
    }

    pub fn ca_cert(der_base64: impl Into<String>, subject: Option<String>) -> Self {
        Self::CaCertificate {
            der_base64: der_base64.into(),
            subject,
        }
    }

    pub fn compute_spki_hash(certificate: &CertificateDer<'_>, algorithm: PinAlgorithm) -> String {
        let data = match parse_spki(certificate) {
            Some(spki) => spki,
            None => certificate.as_ref().to_vec(), // fallback: hash raw bytes if not valid DER
        };
        let digest = algorithm.digest(&data);
        base64::engine::general_purpose::STANDARD.encode(&digest)
    }

    pub fn matches(&self, certificate: &CertificateDer<'_>) -> bool {
        match self {
            Self::SpkiHash { algorithm, hash } => {
                let computed = Self::compute_spki_hash(certificate, *algorithm);
                let normalized = normalize_base64_hash(hash);
                normalized == computed
            }
            Self::CaCertificate { der_base64, .. } => {
                let decoded = match base64::engine::general_purpose::STANDARD.decode(der_base64) {
                    Ok(d) => d,
                    Err(_) => return false,
                };
                certificate.as_ref() == decoded.as_slice()
            }
        }
    }

    pub fn matches_chain(&self, chain: &[CertificateDer<'_>]) -> bool {
        chain.iter().any(|cert| self.matches(cert))
    }
}

fn normalize_base64_hash(hash: &str) -> String {
    let trimmed = hash.trim();
    let replaced = trimmed.replace("-", "+").replace("_", "/");
    let needs_padding = replaced.len() % 4 != 0;
    if needs_padding {
        let pad = 4 - (replaced.len() % 4);
        format!("{}{}", replaced, "=".repeat(pad))
    } else {
        replaced
    }
}

fn parse_spki(certificate: &CertificateDer<'_>) -> Option<Vec<u8>> {
    asn1_parse_spki(certificate.as_ref())
}

fn asn1_parse_spki(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;

    let (tag, len, consumed) = read_tlv(data, pos)?;
    if tag != 0x30 {
        return None;
    }
    pos += consumed;

    if pos + len > data.len() {
        return None;
    }

    let tbs_end = pos + len;

    let (tbs_tag, tbs_len, tbs_consumed) = read_tlv(data, pos)?;
    if tbs_tag != 0x30 {
        return None;
    }
    pos += tbs_consumed;

    let inner_end = pos + tbs_len;
    if inner_end > tbs_end {
        return None;
    }

    while pos < inner_end {
        let (field_tag, field_len, field_consumed) = read_tlv(data, pos)?;
        pos += field_consumed;

        match field_tag {
            0x30 if pos + field_len <= inner_end => {
                while pos < pos + field_len {
                    let (inner_tag, inner_len, inner_consumed) = read_tlv(data, pos)?;
                    pos += inner_consumed;

                    if inner_tag == 0x06 {
                        if pos + inner_len <= inner_end {
                            let oid = &data[pos..pos + inner_len];

                            const SPKI_OID: &[u8] = &[0x55, 0x1d, 0x23];
                            if oid == SPKI_OID {
                                pos += inner_len;
                                while pos < pos + 0 {
                                    let (ext_tag, ext_len, ext_consumed) = read_tlv(data, pos)?;
                                    pos += ext_consumed;
                                    if ext_tag == 0x03 {
                                        if ext_len >= 3 {
                                            let skip = ext_len - 3;
                                            pos += skip;
                                            return Some(data[pos..pos + field_len].to_vec());
                                        }
                                    }
                                    pos += ext_len;
                                }
                            }

                            pos += inner_len;
                        }
                    } else {
                        pos += inner_len;
                    }
                }
            }
            _ => {
                pos += field_len;
            }
        }
    }

    None
}

fn read_tlv(data: &[u8], pos: usize) -> Option<(u8, usize, usize)> {
    if pos >= data.len() {
        return None;
    }
    let tag = data[pos];
    let mut idx = pos + 1;

    if idx >= data.len() {
        return None;
    }
    let first = data[idx];
    idx += 1;

    let len = if first & 0x80 == 0 {
        first as usize
    } else {
        let num_bytes = (first & 0x7f) as usize;
        if num_bytes == 0 || num_bytes > 4 || idx + num_bytes > data.len() {
            return None;
        }
        let mut length = 0usize;
        for &b in &data[idx..idx + num_bytes] {
            length = length.checked_mul(256)? + b as usize;
        }
        idx += num_bytes;
        length
    };

    Some((tag, len, idx - pos))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinMatchPolicy {
    RequireAll,
    RequireAny,
}

impl Default for PinMatchPolicy {
    fn default() -> Self {
        Self::RequireAny
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificatePinningConfig {
    pub pins: HashMap<String, Vec<CertPin>>,
    pub default_pins: Vec<CertPin>,
    pub policy: PinMatchPolicy,
    pub enforce: bool,
}

impl Default for CertificatePinningConfig {
    fn default() -> Self {
        Self {
            pins: HashMap::new(),
            default_pins: Vec::new(),
            policy: PinMatchPolicy::RequireAny,
            enforce: true,
        }
    }
}

impl CertificatePinningConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(mut self, policy: PinMatchPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn enforce_on_failure(mut self, enforce: bool) -> Self {
        self.enforce = enforce;
        self
    }

    pub fn add_pin_for_host(mut self, host: &str, pin: CertPin) -> Self {
        self.pins.entry(host.to_lowercase()).or_default().push(pin);
        self
    }

    pub fn add_default_pin(mut self, pin: CertPin) -> Self {
        self.default_pins.push(pin);
        self
    }

    pub fn has_pins_for_host(&self, host: &str) -> bool {
        let host_lower = host.to_lowercase();
        self.pins.contains_key(&host_lower) || !self.default_pins.is_empty()
    }

    pub fn get_pins_for_host(&self, host: &str) -> &[CertPin] {
        let host_lower = host.to_lowercase();
        if let Some(pins) = self.pins.get(&host_lower) {
            return pins;
        }
        &self.default_pins
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("certificate pinning verification failed for {host}: no matching pins")]
    PinVerificationFailed { host: String },
    #[error("certificate pinning misconfiguration: {0}")]
    Misconfiguration(String),
    #[error("TLS error: {0}")]
    Tls(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct CertVerifier {
    config: Arc<CertificatePinningConfig>,
}

impl CertVerifier {
    pub fn new(config: Arc<CertificatePinningConfig>) -> Self {
        Self { config }
    }

    fn verify(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        host: &str,
    ) -> Result<(), tokio_rustls::rustls::Error> {
        if !self.config.has_pins_for_host(host) {
            return Ok(());
        }

        let pins = self.config.get_pins_for_host(host);
        if pins.is_empty() {
            return Ok(());
        }

        let full_chain: Vec<CertificateDer<'_>> = {
            let mut chain = vec![end_entity.clone()];
            chain.extend(intermediates.iter().cloned());
            chain
        };

        let match_policy = self.config.policy;

        let any_match = match match_policy {
            PinMatchPolicy::RequireAny => pins.iter().any(|pin| pin.matches_chain(&full_chain)),
            PinMatchPolicy::RequireAll => pins.iter().all(|pin| pin.matches_chain(&full_chain)),
        };

        if any_match {
            Ok(())
        } else {
            Err(tokio_rustls::rustls::Error::General(String::from(
                "certificate pinning verification failed",
            )))
        }
    }
}

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for CertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error>
    {
        let host = match server_name {
            ServerName::DnsName(dns) => dns.as_ref().to_string(),
            _ => String::new(),
        };

        self.verify(end_entity, intermediates, &host)?;
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        vec![
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA512,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA512,
            tokio_rustls::rustls::SignatureScheme::ED25519,
        ]
    }
}

pub fn build_tls_connector(
    config: &CertificatePinningConfig,
) -> Result<tokio_rustls::TlsConnector, TlsError> {
    let tls_config = tokio_rustls::rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(CertVerifier::new(Arc::new(config.clone()))))
        .with_no_client_auth();

    Ok(tokio_rustls::TlsConnector::from(Arc::new(tls_config)))
}

pub fn pinned_client_builder(
    client_builder: reqwest::ClientBuilder,
    _config: &CertificatePinningConfig,
) -> Result<reqwest::ClientBuilder, TlsError> {
    // Note: reqwest 0.12 removed use_preconfigured_tls.
    // Certificate pinning with custom verifier is not directly supported.
    // Use add_root_certificate() + tls_built_in_root_certs(false) for basic pinning.
    tracing::warn!("Certificate pinning with custom verifier is not supported in reqwest 0.12");
    Ok(client_builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn test_normalize_base64_hash_url_safe() {
        let normalized = normalize_base64_hash("aBcDeFg");
        assert_eq!(normalized, "aBcDeFg=");
    }

    #[test]
    fn test_normalize_base64_hash_already_padded() {
        let normalized = normalize_base64_hash("aBcD");
        assert_eq!(normalized, "aBcD");
    }

    #[test]
    fn test_normalize_base64_hash_url_safe_no_pad() {
        let normalized = normalize_base64_hash("YWJjZGVmZ2hpams");
        assert_eq!(normalized, "YWJjZGVmZ2hpams=");
    }

    #[test]
    fn test_normalize_base64_hash_with_dashes() {
        let normalized = normalize_base64_hash("abc-def_ghi");
        assert_eq!(normalized, "abc+def/ghi=");
    }

    #[test]
    fn test_pin_algorithm_hash_length() {
        assert_eq!(PinAlgorithm::Sha256.hash_length(), 32);
        assert_eq!(PinAlgorithm::Sha384.hash_length(), 48);
        assert_eq!(PinAlgorithm::Sha512.hash_length(), 64);
    }

    #[test]
    fn test_pin_algorithm_digest() {
        let data = b"test data";
        let sha256 = PinAlgorithm::Sha256.digest(data);
        let expected = sha2::Sha256::digest(data);
        assert_eq!(sha256, expected.to_vec());

        let sha512 = PinAlgorithm::Sha512.digest(data);
        let expected = sha2::Sha512::digest(data);
        assert_eq!(sha512, expected.to_vec());
    }

    #[test]
    fn test_cert_pin_matches_spki() {
        let spki_data = b"some-spki-data-for-testing";
        let hash = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(spki_data));

        let cert = CertificateDer::from(spki_data.to_vec());
        let computed = CertPin::compute_spki_hash(&cert, PinAlgorithm::Sha256);

        match computed.as_str() {
            "" => {
                let normalized = normalize_base64_hash(&hash);
                assert_ne!(normalized, "", "hash should not be empty");
            }
            computed_hash => {
                let normalized = normalize_base64_hash(&hash);
                assert_eq!(normalized, computed_hash);
            }
        }
    }

    #[test]
    fn test_cert_pin_spki_mismatch() {
        let hash = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(b"completely-different-data"));

        let pin = CertPin::spki_sha256(&hash);
        let cert = CertificateDer::from(b"some-spki-data-for-testing".to_vec());
        let computed = CertPin::compute_spki_hash(&cert, PinAlgorithm::Sha256);
        let normalized = normalize_base64_hash(&hash);
        assert_ne!(normalized, computed);
    }

    #[test]
    fn test_cert_pin_spki_mismatch_direct() {
        let hash = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(b"completely-different-data"));

        let pin = CertPin::spki_sha256(&hash);
        assert!(!pin.matches(&CertificateDer::from(
            b"some-spki-data-for-testing".to_vec()
        )));
    }

    #[test]
    fn test_cert_pin_ca_cert_match() {
        let cert_der = b"fake-cert-data";
        let b64 = base64::engine::general_purpose::STANDARD.encode(cert_der);
        let pin = CertPin::ca_cert(&b64, Some("Test CA".to_string()));
        assert!(pin.matches(&CertificateDer::from(cert_der.to_vec())));
    }

    #[test]
    fn test_cert_pin_ca_cert_mismatch() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"other-cert-data");
        let pin = CertPin::ca_cert(&b64, None);
        assert!(!pin.matches(&CertificateDer::from(b"fake-cert-data".to_vec())));
    }

    #[test]
    fn test_cert_pin_matches_chain() {
        let target_spki = b"target-spki";
        let b64 = base64::engine::general_purpose::STANDARD.encode(target_spki);
        let pin = CertPin::ca_cert(&b64, None);
        let chain = vec![
            CertificateDer::from(b"first-cert".to_vec()),
            CertificateDer::from(target_spki.to_vec()),
            CertificateDer::from(b"third-cert".to_vec()),
        ];
        assert!(pin.matches_chain(&chain));
    }

    #[test]
    fn test_cert_pin_no_match_in_chain() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"not-in-chain");
        let pin = CertPin::ca_cert(&b64, None);
        let chain = vec![
            CertificateDer::from(b"first-cert".to_vec()),
            CertificateDer::from(b"second-cert".to_vec()),
        ];
        assert!(!pin.matches_chain(&chain));
    }

    #[test]
    fn test_cert_pin_no_match_in_chain_spki() {
        let hash = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(b"not-in-chain"));

        let pin = CertPin::spki_sha256(&hash);
        let chain = vec![
            CertificateDer::from(b"first-cert".to_vec()),
            CertificateDer::from(b"second-cert".to_vec()),
        ];
        assert!(!pin.matches_chain(&chain));
    }

    #[test]
    fn test_pinning_config_host_specific() {
        let config = CertificatePinningConfig::new()
            .add_pin_for_host("example.com", CertPin::spki_sha256("hash1"))
            .add_pin_for_host("api.example.com", CertPin::spki_sha256("hash2"));

        assert!(config.has_pins_for_host("example.com"));
        assert!(config.has_pins_for_host("api.example.com"));
        assert!(!config.has_pins_for_host("other.com"));

        assert_eq!(config.get_pins_for_host("example.com").len(), 1);
        assert_eq!(config.get_pins_for_host("api.example.com").len(), 1);
    }

    #[test]
    fn test_pinning_config_default_pins() {
        let config = CertificatePinningConfig::new()
            .add_default_pin(CertPin::spki_sha256("default-hash"))
            .add_default_pin(CertPin::spki_sha256("default-hash2"));

        assert!(config.has_pins_for_host("any-host.com"));
        assert_eq!(config.get_pins_for_host("any-host.com").len(), 2);
    }

    #[test]
    fn test_pinning_config_host_overrides_default() {
        let config = CertificatePinningConfig::new()
            .add_default_pin(CertPin::spki_sha256("default-hash"))
            .add_pin_for_host("example.com", CertPin::spki_sha256("host-hash"));

        assert_eq!(config.get_pins_for_host("other.com").len(), 1);
        assert_eq!(config.get_pins_for_host("example.com").len(), 1);
    }

    #[test]
    fn test_pinning_config_case_insensitive_host() {
        let config = CertificatePinningConfig::new()
            .add_pin_for_host("Example.COM", CertPin::spki_sha256("hash"));

        assert!(config.has_pins_for_host("example.com"));
        assert!(config.has_pins_for_host("EXAMPLE.COM"));
    }

    #[test]
    fn test_pinning_config_builder_pattern() {
        let config = CertificatePinningConfig::new()
            .with_policy(PinMatchPolicy::RequireAll)
            .enforce_on_failure(true)
            .add_pin_for_host("secure.example.com", CertPin::spki_sha256("hash1"))
            .add_pin_for_host("secure.example.com", CertPin::spki_sha256("hash2"))
            .add_default_pin(CertPin::spki_sha256("fallback-hash"));

        assert_eq!(config.policy, PinMatchPolicy::RequireAll);
        assert!(config.enforce);
        assert_eq!(config.get_pins_for_host("secure.example.com").len(), 2);
    }

    #[test]
    fn test_pin_match_policy_require_any() {
        let pin1 = CertPin::ca_cert(
            &base64::engine::general_purpose::STANDARD.encode(b"hash1"),
            None,
        );
        let pin2 = CertPin::ca_cert(
            &base64::engine::general_purpose::STANDARD.encode(b"hash2"),
            None,
        );
        let matching_pin = CertPin::ca_cert(
            &base64::engine::general_purpose::STANDARD.encode(b"target-data"),
            None,
        );

        let pins = vec![pin1, pin2, matching_pin];
        let chain = vec![CertificateDer::from(b"target-data".to_vec())];

        let any_match = pins.iter().any(|pin| pin.matches_chain(&chain));
        assert!(any_match);
    }

    #[test]
    fn test_pin_match_policy_require_all() {
        let pin1 = CertPin::ca_cert(
            &base64::engine::general_purpose::STANDARD.encode(b"data1"),
            None,
        );
        let pin2 = CertPin::ca_cert(
            &base64::engine::general_purpose::STANDARD.encode(b"data2"),
            None,
        );

        let pins = vec![pin1, pin2];
        let chain = vec![
            CertificateDer::from(b"data1".to_vec()),
            CertificateDer::from(b"data2".to_vec()),
        ];

        let all_match = pins.iter().all(|pin| pin.matches_chain(&chain));
        assert!(all_match);
    }

    #[test]
    fn test_read_tlv_simple() {
        let data = [0x30, 0x05, 0x01, 0x01, 0xFF, 0x02, 0x01, 0x00];
        let (tag, len, consumed) = read_tlv(&data, 0).unwrap();
        assert_eq!(tag, 0x30);
        assert_eq!(len, 5);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_read_tlv_long_form() {
        let data = [0x30, 0x82, 0x01, 0x00, 0x00];
        let (tag, len, consumed) = read_tlv(&data, 0).unwrap();
        assert_eq!(tag, 0x30);
        assert_eq!(len, 256);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_read_tlv_out_of_bounds() {
        assert!(read_tlv(&[0x30], 0).is_none());
    }

    #[test]
    fn test_serde_roundtrip_spki_pin() {
        let pin = CertPin::spki_sha256("AAAAAAAAAAAAAAAAAAAAAA");
        let json = serde_json::to_string(&pin).unwrap();
        let deserialized: CertPin = serde_json::from_str(&json).unwrap();
        match deserialized {
            CertPin::SpkiHash { algorithm, hash } => {
                assert_eq!(algorithm, PinAlgorithm::Sha256);
                assert_eq!(hash, "AAAAAAAAAAAAAAAAAAAAAA");
            }
            _ => panic!("expected SpkiHash variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_ca_pin() {
        let pin = CertPin::ca_cert("SGVsbG8gV29ybGQ=", Some("Test CA".to_string()));
        let json = serde_json::to_string(&pin).unwrap();
        let deserialized: CertPin = serde_json::from_str(&json).unwrap();
        match deserialized {
            CertPin::CaCertificate {
                der_base64,
                subject,
            } => {
                assert_eq!(der_base64, "SGVsbG8gV29ybGQ=");
                assert_eq!(subject, Some("Test CA".to_string()));
            }
            _ => panic!("expected CaCertificate variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_pinning_config() {
        let config = CertificatePinningConfig::new()
            .with_policy(PinMatchPolicy::RequireAny)
            .add_pin_for_host("example.com", CertPin::spki_sha256("hash1"))
            .add_default_pin(CertPin::spki_sha256("hash2"));

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CertificatePinningConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.has_pins_for_host("example.com"));
        assert!(deserialized.has_pins_for_host("other.com"));
    }
}
