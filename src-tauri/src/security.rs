//! Central security controls for outbound HTTP, error/log redaction and
//! authenticated encryption. Keep all secret-bearing diagnostics behind this
//! module so a new provider cannot accidentally disclose credentials.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use regex::{Captures, Regex};
use std::sync::LazyLock;
use std::time::Duration;
use zeroize::Zeroizing;

static SECRET_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ix)
        (authorization|proxy-authorization|x-api-key|api[_-]?key|access[_-]?token|refresh[_-]?token|token|cookie|session|password|secret)
        (\s*[:=]\s*)
        (?:bearer\s+)?
        [^\s,;\"'&}]+"#,
    )
    .expect("security redaction regex")
});

static BEARER_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{4,}").expect("bearer redaction regex")
});

static OPENAI_STYLE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bsk-[A-Za-z0-9_-]{4,}").expect("key redaction regex"));

/// Unified sensitive-data filter used for UI errors and application logs.
pub struct SensitiveDataFilter;

impl SensitiveDataFilter {
    pub fn redact(input: &str) -> String {
        let fields = SECRET_VALUE.replace_all(input, |caps: &Captures<'_>| {
            format!("{}{}******", &caps[1], &caps[2])
        });
        let bearer = BEARER_VALUE.replace_all(&fields, "Bearer ******");
        OPENAI_STYLE_KEY
            .replace_all(&bearer, |caps: &Captures<'_>| mask_secret(&caps[0]))
            .into_owned()
    }
}

/// Preserve only a recognizable prefix and the last four characters.
pub fn mask_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let prefix = if trimmed.starts_with("sk-") {
        "sk-"
    } else {
        ""
    };
    format!("{prefix}****{suffix}")
}

/// A credential-bearing HTTP client with TLS verification enabled, HTTPS-only
/// transport, no implicit proxy and no redirects that could forward headers.
pub fn secure_http_client(timeout_secs: u64) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
}

pub fn custom_endpoint_origin(endpoint: &str) -> Result<String, String> {
    let url = url::Url::parse(endpoint).map_err(|_| "自定义网关 URL 无效".to_string())?;
    if url.scheme() != "https" {
        return Err("自定义网关必须使用 HTTPS".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("自定义网关 URL 禁止包含用户名或密码".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("自定义网关 Base URL 禁止包含查询参数或片段".into());
    }
    let domain = match url.host() {
        Some(url::Host::Domain(domain)) => domain.trim_end_matches('.').to_ascii_lowercase(),
        Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_)) => {
            return Err("自定义网关禁止使用 IP 地址，请使用可验证的公网域名".into());
        }
        None => return Err("自定义网关缺少域名".into()),
    };
    if !domain.contains('.')
        || domain == "localhost"
        || domain.ends_with(".localhost")
        || domain.ends_with(".local")
        || domain.ends_with(".internal")
        || domain.ends_with(".home")
    {
        return Err("自定义网关禁止使用本机或内部网络域名".into());
    }
    Ok(url.origin().ascii_serialization())
}

fn is_public_ip(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            let octets = ip.octets();
            let shared_or_special = (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19));
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || octets[0] == 0
                || octets[0] >= 224
                || shared_or_special)
        }
        std::net::IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_ip(std::net::IpAddr::V4(mapped));
            }
            let segments = ip.segments();
            let special = segments[0] == 0
                || (segments[0] == 0x0064 && segments[1] == 0xff9b)
                || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
                || (segments[0] == 0x2001 && segments[1] == 0x0db8);
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || special)
        }
    }
}

/// Resolve and pin a custom endpoint to public addresses for this client. This
/// prevents a second DNS lookup from switching the destination to a private
/// network after validation (DNS rebinding).
pub async fn secure_http_client_for_custom_endpoint(
    endpoint: &str,
    timeout_secs: u64,
) -> Result<reqwest::Client, String> {
    custom_endpoint_origin(endpoint)?;
    let url = url::Url::parse(endpoint).map_err(|_| "自定义网关 URL 无效".to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "自定义网关缺少域名".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "无法解析自定义网关域名".to_string())?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("自定义网关解析到本机、私网或保留地址，已拒绝连接".into());
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| "无法创建安全网络客户端".into())
}

/// Remote response bodies are untrusted and may echo submitted credentials.
/// Return only a bounded, non-secret status classification to the UI/logs.
pub fn safe_http_status_error(status: reqwest::StatusCode) -> String {
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            format!("HTTP {status}: authentication failed")
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => format!("HTTP {status}: rate limited"),
        _ if status.is_server_error() => format!("HTTP {status}: provider unavailable"),
        _ => format!("HTTP {status}: request rejected"),
    }
}

pub fn safe_log(scope: &str, message: impl AsRef<str>) {
    eprintln!(
        "[{scope}] {}",
        SensitiveDataFilter::redact(message.as_ref())
    );
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("invalid AES-256 key length")]
    InvalidKey,
    #[error("sensitive data encryption failed")]
    Encrypt,
    #[error("sensitive data decryption failed")]
    Decrypt,
}

/// AES-256-GCM field encryption. The 32-byte key must come from the platform
/// keyring; ciphertext storage alone never contains key material.
pub fn encrypt_sensitive(
    key: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; 12]), EncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| EncryptionError::InvalidKey)?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| EncryptionError::Encrypt)?;
    Ok((ciphertext, nonce_bytes))
}

pub fn decrypt_sensitive(
    key: &[u8],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, EncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| EncryptionError::InvalidKey)?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map(Zeroizing::new)
        .map_err(|_| EncryptionError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_forms() {
        let input = "Authorization: Bearer abc.def.ghi token=xyz cookie=session123 password=hunter2 sk-project-12345678";
        let redacted = SensitiveDataFilter::redact(input);
        for secret in [
            "abc.def.ghi",
            "xyz",
            "session123",
            "hunter2",
            "project-1234",
        ] {
            assert!(!redacted.contains(secret));
        }
        assert!(redacted.contains("******"));
    }

    #[test]
    fn masks_key_for_ui() {
        assert_eq!(mask_secret("sk-example1234"), "sk-****1234");
        assert_eq!(mask_secret("abcd1234"), "****1234");
    }

    #[test]
    fn aes_gcm_round_trip_and_tamper_detection() {
        let key = [7u8; 32];
        let (mut encrypted, nonce) = encrypt_sensitive(&key, b"token-value").unwrap();
        let clear = decrypt_sensitive(&key, &nonce, &encrypted).unwrap();
        assert_eq!(clear.as_slice(), b"token-value");
        encrypted[0] ^= 1;
        assert!(decrypt_sensitive(&key, &nonce, &encrypted).is_err());
    }

    #[test]
    fn floating_window_source_has_no_secret_fields() {
        let source = include_str!("../../src/components/MiniBall.tsx").to_ascii_lowercase();
        for field in [
            "apikey",
            "api_key",
            "access_token",
            "refresh_token",
            "password",
            "cookie",
        ] {
            assert!(
                !source.contains(field),
                "floating window exposes forbidden field: {field}"
            );
        }
    }

    #[test]
    fn custom_endpoints_reject_local_and_ip_targets() {
        for endpoint in [
            "https://127.0.0.1",
            "https://[::1]",
            "https://localhost",
            "https://service.local",
            "https://gateway.internal",
        ] {
            assert!(
                custom_endpoint_origin(endpoint).is_err(),
                "accepted {endpoint}"
            );
        }
        assert_eq!(
            custom_endpoint_origin("https://gateway.example.com/v1").unwrap(),
            "https://gateway.example.com"
        );
    }

    #[test]
    fn private_ip_classifier_is_fail_closed() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "198.18.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
        ] {
            assert!(!is_public_ip(ip.parse().unwrap()));
        }
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
