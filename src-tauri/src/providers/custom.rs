//! 通用自定义 API Provider 适配器。
//!
//! 通过用户可配置的 HTTP 请求（方法 / URL / Query / Headers / Body / 认证）
//! 与 JSON 点路径响应映射，把任意返回余额/额度/用量/重置时间的接口接入
//! 统一的 `ProviderUsage` 展示与刷新链路。
//!
//! 安全边界：
//! - 敏感值（Bearer Token / API Key / Basic Auth 密码 / 自定义 Header 值）
//!   只经 `SecureStorage` 存入系统 keyring，绝不落 SQLite；本模块仅从调用方
//!   传入的 `secret` 参数读取，不持久化。
//! - 非敏感配置（method/url/query/headers/body/auth 结构/响应映射/单位）以
//!   JSON 存入 `providers.custom_config`。
//! - 错误信息、测试连接响应预览均经 `security::SensitiveDataFilter` 脱敏。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 自定义 API 的单位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CustomUnit {
    Token,
    Count,
    Currency,
    Custom,
}

impl CustomUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            CustomUnit::Token => "token",
            CustomUnit::Count => "count",
            CustomUnit::Currency => "currency",
            CustomUnit::Custom => "custom",
        }
    }
}

/// 认证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CustomAuthType {
    Bearer,
    ApiKey,
    Basic,
    #[default]
    None,
    CustomHeader,
}

/// 认证配置（敏感值不在此结构内，由 keyring 承载）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAuth {
    #[serde(rename = "type", default)]
    pub auth_type: CustomAuthType,
    /// ApiKey / CustomHeader 使用的请求头名；默认 `X-API-Key`。
    #[serde(default)]
    pub header_name: Option<String>,
    /// Basic Auth 的用户名（非敏感）。
    #[serde(default)]
    pub username: Option<String>,
}

/// 响应字段映射（JSON 点路径）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomResponseMapping {
    #[serde(default)]
    pub remaining_path: Option<String>,
    #[serde(default)]
    pub total_path: Option<String>,
    #[serde(default)]
    pub used_path: Option<String>,
    #[serde(default)]
    pub reset_time_path: Option<String>,
}

/// 键值对（用于 query 与 headers）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomKeyValue {
    pub key: String,
    pub value: String,
}

/// 完整自定义 API 配置（非敏感，可落 SQLite）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomApiConfig {
    /// 请求 URL（完整地址，含路径、不含 query）。
    pub url: String,
    /// HTTP 方法（GET / POST）。
    pub method: String,
    #[serde(default)]
    pub query: Vec<CustomKeyValue>,
    #[serde(default)]
    pub headers: Vec<CustomKeyValue>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub auth: CustomAuth,
    #[serde(default)]
    pub response_mapping: CustomResponseMapping,
    #[serde(default)]
    pub unit: Option<CustomUnit>,
}

/// 解析配置 JSON；失败返回脱敏后的可读错误。
pub fn parse_custom_config(json: &str) -> Result<CustomApiConfig, ProviderError> {
    let json = json.trim();
    if json.is_empty() {
        return Err(ProviderError::Api("自定义 API 配置为空".into()));
    }
    let config: CustomApiConfig = serde_json::from_str(json)
        .map_err(|e| ProviderError::Api(format!("自定义 API 配置无效: {e}")))?;
    validate_config(&config)?;
    Ok(config)
}

/// 配置结构校验（不发起网络请求）。
pub fn validate_config(config: &CustomApiConfig) -> Result<(), ProviderError> {
    let url = url::Url::parse(&config.url)
        .map_err(|_| ProviderError::Api("自定义 API 请求 URL 无效".into()))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(ProviderError::Api(
            "自定义 API 请求 URL 仅支持 http/https".into(),
        ));
    }
    if url.host_str().is_none() {
        return Err(ProviderError::Api("自定义 API 请求 URL 缺少主机名".into()));
    }
    method_from_str(&config.method)?;
    if config.query.iter().any(|kv| kv.key.trim().is_empty()) {
        return Err(ProviderError::Api("Query 参数名不能为空".into()));
    }
    if config.headers.iter().any(|kv| kv.key.trim().is_empty()) {
        return Err(ProviderError::Api("Header 名不能为空".into()));
    }
    let mapping = &config.response_mapping;
    if mapping.remaining_path.is_none()
        && mapping.total_path.is_none()
        && mapping.used_path.is_none()
        && mapping.reset_time_path.is_none()
    {
        return Err(ProviderError::Api(
            "至少需要配置一个响应字段映射（remaining/total/used/resetTime）".into(),
        ));
    }
    Ok(())
}

/// 解析 HTTP 方法；仅允许 GET / POST。
fn method_from_str(method: &str) -> Result<reqwest::Method, ProviderError> {
    match method.trim().to_ascii_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        other => Err(ProviderError::Api(format!("不支持的 HTTP 方法: {other}"))),
    }
}

/// 安全 JSON 点路径读取（支持嵌套 object 与数组索引，如 `data.items.0.value`）。
/// 不执行任何表达式求值。
pub fn json_path_get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in path.split('.') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        match current {
            Value::Object(map) => current = map.get(segment)?,
            Value::Array(array) => {
                let index: usize = segment.parse().ok()?;
                current = array.get(index)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// 把字段值解析为非负有限数值（接受数字或数字字符串）。
pub fn parse_non_negative_number(value: &Value) -> Result<f64, ProviderError> {
    let number = match value {
        Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| ProviderError::Api("数值溢出".into()))?,
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Err(ProviderError::Api("字段为数字字符串但为空".into()));
            }
            text.parse::<f64>()
                .map_err(|_| ProviderError::Api(format!("字段不是数字: {}", truncate(text))))?
        }
        _ => return Err(ProviderError::Api("字段类型不是数字或数字字符串".into())),
    };
    if !number.is_finite() {
        return Err(ProviderError::Api("字段数值非有限（NaN/Infinity）".into()));
    }
    if number < 0.0 {
        return Err(ProviderError::Api("字段数值为负".into()));
    }
    Ok(number)
}

/// 把字段值解析为重置时间字符串（接受字符串或数字时间戳）。
pub fn parse_reset_time(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// 截断长字符串用于错误信息，避免把响应回显进日志/错误。
fn truncate(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() > 32 {
        let head: String = text.chars().take(32).collect();
        format!("{head}…")
    } else {
        text.to_owned()
    }
}

/// 一次解析后的中间结果。
#[derive(Debug, Clone, Default)]
pub struct CustomParsed {
    pub status: u16,
    pub remaining: Option<f64>,
    pub total: Option<f64>,
    pub used: Option<f64>,
    pub reset_time: Option<String>,
}

/// 按响应映射从 JSON 中提取字段，并按余额计算规则推导 remaining。
pub fn map_response(
    root: &Value,
    mapping: &CustomResponseMapping,
) -> Result<CustomParsed, ProviderError> {
    let read = |path: &Option<String>| -> Result<Option<f64>, ProviderError> {
        match path {
            Some(path) if !path.trim().is_empty() => {
                let value = json_path_get(root, path)
                    .ok_or_else(|| ProviderError::Api(format!("响应缺少字段: {path}")))?;
                Ok(Some(parse_non_negative_number(value)?))
            }
            _ => Ok(None),
        }
    };

    let remaining = read(&mapping.remaining_path)?;
    let total = read(&mapping.total_path)?;
    let used = read(&mapping.used_path)?;

    // 余额计算规则：remaining 缺失时用 total - used 推导。
    let remaining = match (remaining, total, used) {
        (Some(r), _, _) => Some(r),
        (None, Some(total), Some(used)) => {
            if used > total {
                return Err(ProviderError::Api(
                    "字段数值非法：used 大于 total，无法推导 remaining".into(),
                ));
            }
            Some(total - used)
        }
        _ => None,
    };

    let reset_time = mapping
        .reset_time_path
        .as_deref()
        .and_then(|path| json_path_get(root, path))
        .and_then(parse_reset_time);

    if remaining.is_none() && total.is_none() && used.is_none() && reset_time.is_none() {
        return Err(ProviderError::Api(
            "响应中未解析到任何有效字段（remaining/total/used/resetTime）".into(),
        ));
    }

    Ok(CustomParsed {
        status: 200,
        remaining,
        total,
        used,
        reset_time,
    })
}

/// 构造带认证的请求构建器。
fn apply_auth(
    builder: reqwest::RequestBuilder,
    auth: &CustomAuth,
    secret: &str,
) -> reqwest::RequestBuilder {
    match auth.auth_type {
        CustomAuthType::Bearer => builder.bearer_auth(secret),
        CustomAuthType::ApiKey => {
            let header = auth
                .header_name
                .as_deref()
                .map(str::trim)
                .filter(|h| !h.is_empty())
                .unwrap_or("X-API-Key");
            builder.header(header, secret)
        }
        CustomAuthType::CustomHeader => match auth
            .header_name
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        {
            Some(header) => builder.header(header, secret),
            None => builder,
        },
        CustomAuthType::Basic => {
            let username = auth.username.as_deref().unwrap_or("");
            use base64::Engine;
            let token =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{secret}"));
            builder.header(reqwest::header::AUTHORIZATION, format!("Basic {token}"))
        }
        CustomAuthType::None => builder,
    }
}

/// 自定义 API 适配器。
pub struct CustomProvider;

/// 自定义 API 适配器（生产 fetch 走 HTTPS + 公网域名固定）。
#[async_trait]
impl ProviderAdapter for CustomProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let cfg = parse_custom_config(config.custom_config.as_deref().unwrap_or(""))?;
        let client = crate::security::secure_http_client_for_custom_endpoint(&cfg.url, 20)
            .await
            .map_err(ProviderError::Http)?;
        self.fetch_usage_with_client(config, api_key, &client).await
    }
}

impl CustomProvider {
    /// 可注入 HTTP 客户端的查询实现（测试用 Mock 服务器；生产走安全客户端）。
    pub(crate) async fn fetch_usage_with_client(
        &self,
        config: &ProviderConfig,
        secret: &str,
        client: &reqwest::Client,
    ) -> Result<ProviderUsage, ProviderError> {
        let cfg = parse_custom_config(config.custom_config.as_deref().unwrap_or(""))?;
        let (parsed, _body) = request_and_parse(&cfg, secret, client).await?;
        let unit = cfg.unit.unwrap_or(CustomUnit::Custom);
        Ok(to_usage(config.provider_type.clone(), &cfg, unit, &parsed))
    }
}

/// 发送请求并解析响应（核心请求逻辑，供 fetch 与 test 复用）。
pub async fn request_and_parse(
    cfg: &CustomApiConfig,
    secret: &str,
    client: &reqwest::Client,
) -> Result<(CustomParsed, Value), ProviderError> {
    let method = method_from_str(&cfg.method)?;
    let is_post = method == reqwest::Method::POST;

    // 用结构化 URL API 构造 query，避免手工拼接未编码字符串。
    let mut url =
        url::Url::parse(&cfg.url).map_err(|e| ProviderError::Api(format!("请求 URL 无效: {e}")))?;
    {
        let mut pairs = url.query_pairs_mut();
        for kv in &cfg.query {
            pairs.append_pair(kv.key.trim(), &kv.value);
        }
    }

    let mut builder = client.request(method, url.clone());
    for kv in &cfg.headers {
        let key = kv.key.trim();
        // 认证相关头由 auth 统一处理，跳过可能泄露的用户输入以免双重发送。
        if is_auth_header(key) {
            continue;
        }
        builder = builder.header(key, &kv.value);
    }
    builder = apply_auth(builder, &cfg.auth, secret);

    if is_post {
        if let Some(body) = cfg.body.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_owned());
        }
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(ProviderError::Api(crate::security::safe_http_status_error(
            status,
        )));
    }

    let body_text = resp
        .text()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;
    let root: Value = serde_json::from_str(&body_text)
        .map_err(|_| ProviderError::Api("响应不是合法 JSON".into()))?;

    let mut parsed = map_response(&root, &cfg.response_mapping)?;
    parsed.status = status.as_u16();
    Ok((parsed, root))
}

/// 把解析结果映射到统一 `ProviderUsage`。
pub fn to_usage(
    provider_type: String,
    _cfg: &CustomApiConfig,
    unit: CustomUnit,
    parsed: &CustomParsed,
) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(provider_type);
    usage.remaining = parsed.remaining;
    usage.reset_time = parsed.reset_time.clone();
    match unit {
        CustomUnit::Currency => {
            // 金额单位：remaining 视为剩余金额，同时作为余额展示。
            usage.balance = parsed.remaining.or(parsed.total);
            usage.currency = String::new();
        }
        CustomUnit::Token => {
            usage.total_tokens = parsed.total.unwrap_or(0.0).floor() as u64;
            usage.today_tokens = parsed.used.map(|v| v.floor() as u64);
        }
        CustomUnit::Count | CustomUnit::Custom => {}
    }
    usage.custom = Some(super::CustomUsageDetails {
        remaining: parsed.remaining,
        total: parsed.total,
        used: parsed.used,
        unit: unit.as_str().to_owned(),
    });
    usage.updated_at = chrono::Utc::now().to_rfc3339();
    usage
}

fn is_auth_header(header: &str) -> bool {
    let normalized = header.trim().to_ascii_lowercase();
    normalized == "authorization"
        || normalized == "proxy-authorization"
        || normalized == "x-api-key"
        || normalized.contains("token")
        || normalized.contains("api-key")
        || normalized.contains("apikey")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(
        remaining: Option<&str>,
        total: Option<&str>,
        used: Option<&str>,
    ) -> CustomResponseMapping {
        CustomResponseMapping {
            remaining_path: remaining.map(str::to_owned),
            total_path: total.map(str::to_owned),
            used_path: used.map(str::to_owned),
            reset_time_path: None,
        }
    }

    #[test]
    fn nested_dot_path_reads_object_and_array() {
        let json: Value = serde_json::from_str(
            r#"{"data":{"quota":{"remaining_tokens":120},"items":[{"value":7}]}}"#,
        )
        .unwrap();
        assert_eq!(
            json_path_get(&json, "data.quota.remaining_tokens").and_then(Value::as_i64),
            Some(120)
        );
        assert_eq!(
            json_path_get(&json, "data.items.0.value").and_then(Value::as_i64),
            Some(7)
        );
        assert!(json_path_get(&json, "data.missing").is_none());
        assert!(json_path_get(&json, "data.items.9").is_none());
    }

    #[test]
    fn parses_numbers_and_rejects_invalid() {
        assert_eq!(
            parse_non_negative_number(&serde_json::json!(42)).unwrap(),
            42.0
        );
        assert_eq!(
            parse_non_negative_number(&serde_json::json!("19.5")).unwrap(),
            19.5
        );
        assert!(parse_non_negative_number(&serde_json::json!(-1)).is_err());
        assert!(parse_non_negative_number(&serde_json::json!("abc")).is_err());
        assert!(parse_non_negative_number(&serde_json::json!(true)).is_err());
    }

    #[test]
    fn remaining_directly_used_when_present() {
        let json: Value =
            serde_json::from_str(r#"{"data":{"remaining":10,"total":50,"used":20}}"#).unwrap();
        let parsed = map_response(
            &json,
            &mapping(
                Some("data.remaining"),
                Some("data.total"),
                Some("data.used"),
            ),
        )
        .unwrap();
        assert_eq!(parsed.remaining, Some(10.0));
    }

    #[test]
    fn remaining_computed_as_total_minus_used() {
        let json: Value = serde_json::from_str(r#"{"data":{"total":50,"used":20}}"#).unwrap();
        let parsed =
            map_response(&json, &mapping(None, Some("data.total"), Some("data.used"))).unwrap();
        assert_eq!(parsed.remaining, Some(30.0));
    }

    #[test]
    fn only_remaining_is_preserved() {
        let json: Value = serde_json::from_str(r#"{"data":{"remaining":7}}"#).unwrap();
        let parsed = map_response(&json, &mapping(Some("data.remaining"), None, None)).unwrap();
        assert_eq!(parsed.remaining, Some(7.0));
        assert_eq!(parsed.total, None);
    }

    #[test]
    fn missing_field_is_explicit_error() {
        let json: Value = serde_json::from_str(r#"{"data":{}}"#).unwrap();
        let result = map_response(&json, &mapping(Some("data.balance"), None, None));
        assert!(matches!(result, Err(ProviderError::Api(m)) if m.contains("缺少字段")));
    }

    #[test]
    fn used_greater_than_total_is_error() {
        let json: Value = serde_json::from_str(r#"{"data":{"total":10,"used":20}}"#).unwrap();
        let result = map_response(&json, &mapping(None, Some("data.total"), Some("data.used")));
        assert!(matches!(result, Err(ProviderError::Api(m)) if m.contains("used 大于 total")));
    }

    #[test]
    fn reset_time_accepts_string_and_number() {
        assert_eq!(
            parse_reset_time(&serde_json::json!("2026-08-15T00:00:00Z")).as_deref(),
            Some("2026-08-15T00:00:00Z")
        );
        assert_eq!(
            parse_reset_time(&serde_json::json!(1711929600)).as_deref(),
            Some("1711929600")
        );
        assert_eq!(parse_reset_time(&serde_json::Value::Null), None);
    }

    #[test]
    fn config_validation_rejects_missing_mapping() {
        let config = CustomApiConfig {
            url: "https://example.com/v1".into(),
            method: "GET".into(),
            query: vec![],
            headers: vec![],
            body: None,
            auth: CustomAuth::default(),
            response_mapping: CustomResponseMapping::default(),
            unit: None,
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn config_validation_rejects_unsupported_method() {
        let config = CustomApiConfig {
            url: "https://example.com/v1".into(),
            method: "DELETE".into(),
            query: vec![],
            headers: vec![],
            body: None,
            auth: CustomAuth::default(),
            response_mapping: mapping(Some("data.remaining"), None, None),
            unit: None,
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn basic_auth_encodes_username_password() {
        let auth = CustomAuth {
            auth_type: CustomAuthType::Basic,
            header_name: None,
            username: Some("user".into()),
        };
        let client = reqwest::Client::new();
        let builder = apply_auth(client.get("https://example.com"), &auth, "p@ss");
        let req = builder.build().unwrap();
        let header = req
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(header, "Basic dXNlcjpwQHNz");
    }
}

/// 测试连接结果（脱敏后返回给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTestResult {
    pub success: bool,
    pub status: Option<u16>,
    pub remaining: Option<f64>,
    pub total: Option<f64>,
    pub used: Option<f64>,
    pub unit: String,
    pub reset_time: Option<String>,
    /// 脱敏后的响应结构（JSON 字符串）。
    pub response_preview: Option<String>,
    /// 脱敏后的错误信息。
    pub error: Option<String>,
}

fn empty_test_result(unit: &str, error: Option<String>) -> CustomTestResult {
    CustomTestResult {
        success: false,
        status: None,
        remaining: None,
        total: None,
        used: None,
        unit: unit.to_owned(),
        reset_time: None,
        response_preview: None,
        error,
    }
}

/// 测试连接：发送请求、解析并脱敏预览；不写入用量历史，不改动任何配置。
pub async fn test_connection(
    config_json: &str,
    secret: &str,
) -> Result<CustomTestResult, ProviderError> {
    let cfg = match parse_custom_config(config_json) {
        Ok(cfg) => cfg,
        Err(e) => {
            return Ok(empty_test_result(
                "",
                Some(crate::security::SensitiveDataFilter::redact(&e.to_string())),
            ));
        }
    };
    let unit = cfg.unit.unwrap_or(CustomUnit::Custom);
    let client = match crate::security::custom_test_http_client(&cfg.url, 20) {
        Ok(client) => client,
        Err(e) => {
            return Ok(empty_test_result(
                unit.as_str(),
                Some(crate::security::SensitiveDataFilter::redact(&e)),
            ));
        }
    };
    match request_and_parse(&cfg, secret, &client).await {
        Ok((parsed, body)) => Ok(CustomTestResult {
            success: true,
            status: Some(parsed.status),
            remaining: parsed.remaining,
            total: parsed.total,
            used: parsed.used,
            unit: unit.as_str().to_owned(),
            reset_time: parsed.reset_time.clone(),
            response_preview: Some(crate::security::redact_json(&body).to_string()),
            error: None,
        }),
        Err(e) => Ok(empty_test_result(
            unit.as_str(),
            Some(crate::security::SensitiveDataFilter::redact(&e.to_string())),
        )),
    }
}
