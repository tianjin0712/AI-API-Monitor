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
    #[serde(default)]
    balance_infos: Vec<BalanceInfo>,
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

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        if let Some(info) = data.balance_infos.first() {
            usage.balance = info.total_balance.parse::<f64>().ok();
            usage.currency = info.currency.clone();
        }
        usage.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(usage)
    }
}
