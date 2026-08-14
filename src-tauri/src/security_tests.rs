//! SEC-001 through SEC-015 regression gates. Runtime/platform-specific checks
//! are complemented by `Security_Test_Report.md` manual verification steps.

fn source(path: &str) -> String {
    match path {
        "storage" => include_str!("storage.rs").to_ascii_lowercase(),
        "db" => include_str!("db/mod.rs").to_ascii_lowercase(),
        "codex" => include_str!("providers/codex.rs").to_ascii_lowercase(),
        "assets" => include_str!("assets.rs").to_ascii_lowercase(),
        "security" => include_str!("security.rs").to_ascii_lowercase(),
        "platform" => include_str!("platform_security.rs").to_ascii_lowercase(),
        "mini" => include_str!("../../src/components/MiniBall.tsx").to_ascii_lowercase(),
        _ => String::new(),
    }
}

#[test]
fn sec_001_api_key_uses_platform_keyring() {
    assert!(source("storage").contains("keyring::entry"));
    assert!(!source("db").contains("api_key         text"));
}

#[test]
fn sec_002_logs_redact_keys() {
    let redacted = crate::security::SensitiveDataFilter::redact("api_key=sk-example12345678");
    assert_eq!(redacted, "api_key=******");
}

#[test]
fn sec_003_codex_does_not_read_cookie_or_auth_stores() {
    let production = source("codex")
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .to_string();
    for call in ["read_to_string(", "std::fs::read(", ".bearer_auth("] {
        assert!(!production.contains(call));
    }
}

#[test]
fn sec_004_codex_only_checks_public_cli_rate_limits() {
    let codex = source("codex");
    // Codex authentication remains owned by the CLI App Server; the monitor
    // only requests the public rate-limit snapshot over its stdio protocol.
    assert!(codex.contains("app-server"));
    assert!(codex.contains("account/ratelimits/read"));
    assert!(codex.contains("stdout(stdio::piped())"));
}

#[test]
fn sec_005_assets_use_opaque_app_resource_urls() {
    let assets = source("assets");
    assert!(assets.contains("app-resource"));
    assert!(!assets.contains("file://"));
}

#[test]
fn sec_006_gif_limits_are_enforced() {
    assert_eq!(crate::assets::MAX_ASSET_BYTES, 20 * 1024 * 1024);
    assert_eq!(crate::assets::MAX_IMAGE_DIMENSION, 4096);
    assert_eq!(crate::assets::MAX_GIF_FRAMES, 300);
}

#[test]
fn sec_007_executable_extensions_are_not_allowlisted() {
    let assets = source("assets");
    let production = assets.split("#[cfg(test)]").next().unwrap();
    for allowed in [
        "\"png\"", "\"jpg\"", "\"jpeg\"", "\"webp\"", "\"gif\"", "\"ico\"", "\"svg\"",
    ] {
        assert!(production.contains(allowed));
    }
    for extension in [
        "\"exe\"", "\"bat\"", "\"cmd\"", "\"dll\"", "\"html\"", "\"js\"",
    ] {
        assert!(!production.contains(extension));
    }
}

#[test]
fn sec_008_http_client_enforces_tls() {
    let security = source("security");
    assert!(security.contains(".https_only(true)"));
    assert!(!security.contains("danger_accept_invalid_certs(true)"));
}

#[test]
fn sec_009_proxy_and_redirect_header_leaks_are_blocked() {
    let security = source("security");
    assert!(security.contains(".no_proxy()"));
    assert!(security.contains("policy::none()"));
}

#[test]
fn sec_010_sensitive_fields_use_aes_256_gcm() {
    let key = [42u8; 32];
    let (ciphertext, nonce) = crate::security::encrypt_sensitive(&key, b"secret").unwrap();
    assert_ne!(ciphertext, b"secret");
    assert_eq!(
        crate::security::decrypt_sensitive(&key, &nonce, &ciphertext)
            .unwrap()
            .as_slice(),
        b"secret"
    );
}

#[test]
fn sec_011_crash_and_ui_errors_are_redacted() {
    let text = crate::security::SensitiveDataFilter::redact(
        "Authorization: Bearer token-value cookie=session-value password=pass-value",
    );
    for secret in ["token-value", "session-value", "pass-value"] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn sec_012_local_files_have_private_permission_controls() {
    let platform = source("platform");
    assert!(platform.contains("0o600"));
    assert!(platform.contains("0o700"));
    assert!(platform.contains("protected_dacl_security_information"));
    assert!(platform.contains("(a;oici;fa;;;ow)"));
}

#[test]
fn sec_013_floating_window_has_no_secret_fields() {
    let mini = source("mini");
    for field in [
        "apikey",
        "access_token",
        "refresh_token",
        "password",
        "cookie",
    ] {
        assert!(!mini.contains(field));
    }
}

#[test]
fn sec_014_updates_are_disabled_without_signature_config() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let updater = &config["plugins"]["updater"];
    let endpoints = updater["endpoints"].as_array().unwrap();
    assert!(endpoints.is_empty() || !updater["pubkey"].as_str().unwrap_or("").is_empty());
    for endpoint in endpoints {
        assert!(endpoint.as_str().unwrap().starts_with("https://"));
    }
}

#[test]
fn sec_015_database_uses_wal_and_versioned_migrations() {
    let db = source("db");
    assert!(db.contains("journal_mode\", \"wal"));
    assert!(db.contains("pragma user_version"));
    assert!(db.contains("secure_settings"));
}
