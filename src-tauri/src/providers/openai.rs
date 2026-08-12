//! OpenAI Provider 适配器
//!
//! Usage API：`GET https://api.openai.com/v1/usage?start_time=<unix>&bucket_width=1d`
//! 响应：
//! ```json
//! { "object": "list", "data": [{
//!   "aggregation_timestamp": 1711929600,
//!   "result": { "input_tokens": 1000, "output_tokens": 500, "total_tokens": 1500 }
//! }] }
//! ```
//! 统计最近 30 天数据：totalTokens 累加、input/output 取最后一天（当日）。
//! 余额/费用端点需付费账号且无统一 API，V0.1 留空。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct OpenAIProvider;

#[derive(Debug, Default, Deserialize)]
struct UsageBucketResult {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct UsageBucket {
    #[serde(default)]
    result: UsageBucketResult,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    #[serde(default)]
    data: Vec<UsageBucket>,
}

#[async_trait]
impl ProviderAdapter for OpenAIProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        // 最近 30 天（bucket_width=1d 按天聚合）
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(30);
        let url = format!(
            "{}/usage?start_time={}&bucket_width=1d",
            config.api_url.trim_end_matches('/'),
            start.timestamp()
        );

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

        let data: UsageResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))?;

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into();
        // 总量 = 30 天累加；当日 = 最后一条 bucket
        usage.total_tokens = data.data.iter().map(|b| b.result.total_tokens).sum();
        if let Some(last) = data.data.last() {
            usage.input_tokens = last.result.input_tokens;
            usage.output_tokens = last.result.output_tokens;
        }
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}
