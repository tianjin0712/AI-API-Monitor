//! Codex Provider 适配器（ChatGPT 订阅额度）
//!
//! 与 OpenAI API Key 不同（mission.md §6 Codex）：额度来自 ChatGPT 账号订阅
//! （Plus/Pro 等计划的 Codex 用量），不是 API 计费。
//!
//! 数据源：复用 Codex CLI 的本地登录凭证（~/.codex/auth.json），请求
//! `GET {base}/wham/rate-limit-reset-credits` 获取额度/重置信息。
//! 端点与字段依据 codex-cli 0.146.0 二进制内嵌的协议确认：
//!   - base 默认 `https://chatgpt.com/backend-api/codex`（可配置覆盖）
//!   - 响应含 rate_limit_reset_credits：{ plan_type, rate_limit, credits,
//!     spend_control, additional_rate_limits, rate_limit_reached_type }
//!   - resets_at / spend_control 提供重置时间与剩余百分比

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct CodexProvider;

/// 默认 base（与 codex-cli 一致）。
pub const DEFAULT_CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";

#[derive(Debug, Deserialize)]
struct RateLimitResetCreditsResponse {
    #[serde(default)]
    rate_limit_reset_credits: Option<RateLimitResetCredits>,
}

#[derive(Debug, Deserialize)]
struct RateLimitResetCredits {
    /// 计划类型（plus/pro/...），暂仅记录（保留字段供未来展示）
    #[allow(dead_code)]
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    spend_control: Option<SpendControl>,
    #[serde(default)]
    credits: Option<Credits>,
}

#[derive(Debug, Deserialize)]
struct SpendControl {
    #[serde(default)]
    limit: Option<f64>,
    #[serde(default)]
    used: Option<f64>,
    #[serde(default)]
    remaining_percent: Option<f64>,
    #[serde(default)]
    resets_at: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Credits {
    #[serde(default)]
    balance: Option<f64>,
    #[serde(default)]
    remaining: Option<f64>,
    /// 总额度（保留字段，供未来百分比展示）
    #[allow(dead_code)]
    #[serde(default)]
    limit: Option<f64>,
}

#[async_trait]
impl ProviderAdapter for CodexProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        _api_key: &str, // Codex 使用 CLI 本地凭证，忽略传入 key
    ) -> Result<ProviderUsage, ProviderError> {
        let access_token = read_cli_access_token()?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let base = if config.api_url.trim().is_empty() {
            DEFAULT_CODEX_BASE.to_string()
        } else {
            config.api_url.trim_end_matches('/').to_string()
        };
        let url = format!("{base}/wham/rate-limit-reset-credits");

        let resp = client
            .get(&url)
            .bearer_auth(access_token)
            .header("User-Agent", "codex-cli/0.146.0")
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("HTTP {status}: {body}")));
        }

        let data: RateLimitResetCreditsResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))?;

        let rlc = data.rate_limit_reset_credits;

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "¥".into(); // ChatGPT 订阅按当地货币；无精确币种时用通用符号
        if let Some(rlc) = &rlc {
            usage.reset_time = rlc
                .spend_control
                .as_ref()
                .and_then(|s| s.resets_at.as_ref())
                .and_then(normalize_timestamp);
            // 剩余额度：优先 credits.balance/remaining，其次 spend_control.remaining_percent
            usage.balance = rlc
                .credits
                .as_ref()
                .and_then(|c| c.balance.or(c.remaining));
            usage.remaining = rlc
                .spend_control
                .as_ref()
                .and_then(|s| s.remaining_percent)
                .or_else(|| {
                    // 无 remaining_percent 时，由 limit/used 推算百分比
                    let s = rlc.spend_control.as_ref()?;
                    let limit = s.limit?;
                    let used = s.used.unwrap_or(0.0);
                    (limit > 0.0).then(|| ((limit - used) / limit * 100.0).max(0.0))
                });
            usage.provider = config.provider_type.clone();
        }
        usage.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(usage)
    }
}

/// 从 Codex CLI 登录文件读取 access_token（复用 CLI 登录态，"直接使用 CLI"）。
fn read_cli_access_token() -> Result<String, ProviderError> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| ProviderError::Api("无法定位用户主目录".into()))?;
    let path = std::path::Path::new(&home).join(".codex").join("auth.json");
    let txt = std::fs::read_to_string(&path).map_err(|e| {
        ProviderError::Api(format!(
            "无法读取 Codex 登录凭证（{path:?}）：{e}。请先运行 `codex login` 登录 ChatGPT"
        ))
    })?;
    let v: serde_json::Value = serde_json::from_str(&txt)
        .map_err(|e| ProviderError::Api(format!("auth.json 解析失败: {e}")))?;
    v["tokens"]["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| ProviderError::Api("auth.json 中缺少 access_token（未登录）".into()))
}

/// resets_at 可能是 Unix 秒（数字）或 ISO8601 字符串，统一为 ISO8601。
fn normalize_timestamp(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Number(n) => {
            let secs = n.as_i64().or_else(|| n.as_u64().map(|u| u as i64))?;
            let dt = chrono::DateTime::from_timestamp(secs, 0)?;
            Some(dt.to_rfc3339())
        }
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rate_limit_reset_credits_with_spend_control() {
        let json = r#"{
            "plan_type": "plus",
            "rate_limit_reset_credits": {
                "plan_type": "plus",
                "rate_limit": {},
                "credits": { "balance": 42.5, "limit": 100.0 },
                "spend_control": {
                    "limit": 100.0,
                    "used": 57.5,
                    "remaining_percent": 42.5,
                    "resets_at": 1755000000
                },
                "additional_rate_limits": [],
                "rate_limit_reached_type": "none"
            }
        }"#;
        let resp: RateLimitResetCreditsResponse = serde_json::from_str(json).expect("parse ok");
        let rlc = resp.rate_limit_reset_credits.expect("has credits");
        assert_eq!(rlc.plan_type.as_deref(), Some("plus"));
        assert_eq!(rlc.credits.as_ref().and_then(|c| c.balance), Some(42.5));
        assert_eq!(
            rlc.spend_control.as_ref().and_then(|s| s.remaining_percent),
            Some(42.5)
        );
    }

    #[test]
    fn normalizes_numeric_resets_at() {
        let v = serde_json::json!(1755000000);
        let s = normalize_timestamp(&v).expect("normalized");
        assert!(s.starts_with("2025-08-"), "unix 秒应转 ISO8601: {s}");
    }

    #[test]
    fn normalizes_iso_resets_at() {
        let v = serde_json::json!("2025-08-13T00:00:00Z");
        assert_eq!(normalize_timestamp(&v).as_deref(), Some("2025-08-13T00:00:00Z"));
    }

    #[test]
    fn missing_blocks_still_parse() {
        // 无 rate_limit_reset_credits 字段时应解析成功（空数据）
        let resp: RateLimitResetCreditsResponse =
            serde_json::from_str(r#"{"foo":1}"#).expect("parse ok");
        assert!(resp.rate_limit_reset_credits.is_none());
    }
}
