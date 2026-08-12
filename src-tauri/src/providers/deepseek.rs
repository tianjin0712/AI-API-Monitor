//! DeepSeek Provider 适配器
//!
//! 官方 API：`GET https://api.deepseek.com/user/balance`（Bearer 认证）
//! 响应：
//! ```json
//! { "is_available": true, "balance_infos": [{
//!   "currency": "CNY",
//!   "total_balance": "110.00",
//!   "granted_balance": "10.00",
//!   "topped_up_balance": "100.00"
//! }] }
//! ```
//! DeepSeek 目前无公开 Token Usage 统计端点，Token/费用字段留空。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct DeepSeekProvider;

#[derive(Debug, Deserialize)]
struct BalanceInfo {
    currency: String,
    #[serde(default)]
    total_balance: String,
}

#[derive(Debug, Deserialize)]
struct BalanceResponse {
    /// None = 响应未提供该字段（视为可用），Some(false) = 明确不可用。
    #[serde(default)]
    is_available: Option<bool>,
    #[serde(default)]
    balance_infos: Vec<BalanceInfo>,
}

/// 多币种选择：优先 CNY，否则取首个条目（纯函数，便于测试）。
fn pick_balance(infos: &[BalanceInfo]) -> Option<&BalanceInfo> {
    infos
        .iter()
        .find(|i| i.currency == "CNY")
        .or_else(|| infos.first())
}

#[async_trait]
impl ProviderAdapter for DeepSeekProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let url = format!("{}/user/balance", config.api_url.trim_end_matches('/'));
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

        let data: BalanceResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))?;

        // 账户不可用状态显式上报（而非静默展示空余额）；
        // None（未提供字段）视为可用，避免网关包装响应误报
        if data.is_available == Some(false) {
            return Err(ProviderError::Api("账户不可用（is_available=false）".into()));
        }

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        if let Some(info) = pick_balance(&data.balance_infos) {
            usage.balance = info.total_balance.parse::<f64>().ok();
            usage.currency = info.currency.clone();
        }
        usage.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_balance_response() {
        let json = r#"{
            "is_available": true,
            "balance_infos": [
                { "currency": "CNY", "total_balance": "48.32", "granted_balance": "8.32", "topped_up_balance": "40.00" }
            ]
        }"#;
        let resp: BalanceResponse = serde_json::from_str(json).expect("parse ok");
        let info = resp.balance_infos.first().expect("has balance info");
        assert_eq!(info.currency, "CNY");
        assert_eq!(info.total_balance, "48.32");
    }

    #[test]
    fn handles_empty_balance_infos() {
        let json = r#"{ "is_available": false, "balance_infos": [] }"#;
        let resp: BalanceResponse = serde_json::from_str(json).expect("parse ok");
        assert!(resp.balance_infos.is_empty());
        assert_eq!(resp.is_available, Some(false));
    }

    #[test]
    fn missing_is_available_is_treated_as_available() {
        let json = r#"{ "balance_infos": [ { "currency": "CNY", "total_balance": "10" } ] }"#;
        let resp: BalanceResponse = serde_json::from_str(json).expect("parse ok");
        assert_eq!(resp.is_available, None, "缺失字段应为 None（视为可用）");
    }

    #[test]
    fn pick_balance_prefers_cny_over_first() {
        let infos = vec![
            BalanceInfo { currency: "USD".into(), total_balance: "5".into() },
            BalanceInfo { currency: "CNY".into(), total_balance: "48.32".into() },
        ];
        let picked = pick_balance(&infos).expect("has info");
        assert_eq!(picked.currency, "CNY");
        assert_eq!(picked.total_balance, "48.32");
    }

    #[test]
    fn pick_balance_falls_back_to_first_when_no_cny() {
        let infos = vec![
            BalanceInfo { currency: "USD".into(), total_balance: "5".into() },
            BalanceInfo { currency: "JPY".into(), total_balance: "700".into() },
        ];
        let picked = pick_balance(&infos).expect("has info");
        assert_eq!(picked.currency, "USD");
    }

    #[test]
    fn pick_balance_empty_returns_none() {
        assert!(pick_balance(&[]).is_none());
    }
}
