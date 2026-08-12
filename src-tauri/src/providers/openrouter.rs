//! OpenRouter Provider 适配器
//!
//! 官方 Limits API：`GET {base}/api/v1/key`（Bearer 认证）
//! base 可使用站点根、`/api/v1` 或完整 `/api/v1/key`，适配器会归一化。
//! 响应 `data` 对象包含 key 剩余额度与用量：
//! ```json
//! { "data": { "label": "...", "limit": 5.0, "limit_reset": "...",
//!   "limit_remaining": 2.5, "usage": 12.3, "usage_daily": 1.2,
//!   "usage_weekly": 4.5, "usage_monthly": 10.1, "is_free_tier": false } }
//! ```
//! 注意：`usage*` 字段是 credits/USD 费用金额，不是 Token 数量（V0.4 复审 P1）。
//! 来源：https://openrouter.ai/docs/api_reference/limits

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct OpenRouterProvider;

/// 默认站点根地址。
pub const DEFAULT_OPENROUTER_BASE: &str = "https://openrouter.ai";

/// 构造 Key 查询 URL（纯函数，兼容站点根、API 根与完整端点）。
pub fn key_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/api/v1/key") {
        base.to_string()
    } else if base.ends_with("/api/v1") {
        format!("{base}/key")
    } else {
        format!("{base}/api/v1/key")
    }
}

#[derive(Debug, Deserialize)]
struct KeyInfo {
    #[serde(default)]
    limit_remaining: Option<f64>,
    #[serde(default)]
    limit_reset: Option<String>,
    /// 累计消耗（USD 费用，非 Token；保留字段）
    #[allow(dead_code)]
    #[serde(default)]
    usage: Option<f64>,
    #[serde(default)]
    usage_daily: Option<f64>,
    #[serde(default)]
    usage_monthly: Option<f64>,
    /// 免费额度标记（保留字段）
    #[allow(dead_code)]
    #[serde(default)]
    is_free_tier: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct KeyResponse {
    data: KeyInfo,
}

#[async_trait]
impl ProviderAdapter for OpenRouterProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let base = if config.api_url.trim().is_empty() {
            DEFAULT_OPENROUTER_BASE
        } else {
            config.api_url.trim_end_matches('/')
        };
        let url = key_url(base);
        let resp = client
            .get(&url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("HTTP {status}: {body}")));
        }

        let data: KeyResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))?;

        // usage* 是 credits/USD 费用，不是 Token；total_tokens 保持 0。
        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into(); // OpenRouter credits 以 USD 计费
        usage.balance = data.data.limit_remaining; // None = 无限额度
        usage.today_cost = data.data.usage_daily;
        usage.month_cost = data.data.usage_monthly;
        usage.reset_time = data.data.limit_reset;
        usage.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_response() {
        let json = r#"{
            "data": {
                "label": "my-key",
                "limit": 5.0,
                "limit_reset": "2025-08-13T00:00:00Z",
                "limit_remaining": 2.5,
                "usage": 12.3,
                "usage_daily": 1.2,
                "usage_weekly": 4.5,
                "usage_monthly": 10.1,
                "is_free_tier": false
            }
        }"#;
        let resp: KeyResponse = serde_json::from_str(json).expect("parse ok");
        assert_eq!(resp.data.limit_remaining, Some(2.5));
        assert_eq!(resp.data.usage_daily, Some(1.2));
        assert_eq!(
            resp.data.limit_reset.as_deref(),
            Some("2025-08-13T00:00:00Z")
        );
    }

    #[test]
    fn parses_missing_optional_fields() {
        // 免费额度/无限余额时 limit_remaining 可能为 null 或缺失
        let json = r#"{ "data": { "label": "free", "is_free_tier": true } }"#;
        let resp: KeyResponse = serde_json::from_str(json).expect("parse ok");
        assert_eq!(resp.data.limit_remaining, None);
    }

    #[test]
    fn key_url_contract_matches_official_endpoint() {
        // V0.4 复审 P0：默认 base（站点根）拼出的 URL 必须是官方端点，不能重复路径
        assert_eq!(
            super::key_url(super::DEFAULT_OPENROUTER_BASE),
            "https://openrouter.ai/api/v1/key"
        );
        // 尾部斜杠容忍
        assert_eq!(
            super::key_url("https://openrouter.ai/"),
            "https://openrouter.ai/api/v1/key"
        );
        // API 根地址与完整端点都只拼一次
        assert_eq!(
            super::key_url("https://openrouter.ai/api/v1"),
            "https://openrouter.ai/api/v1/key"
        );
        assert_eq!(
            super::key_url("https://openrouter.ai/api/v1/key/"),
            "https://openrouter.ai/api/v1/key"
        );
        // 自定义网关路径仍以同一规则追加
        assert_eq!(
            super::key_url("https://gateway.example/openrouter"),
            "https://gateway.example/openrouter/api/v1/key"
        );
    }
}
