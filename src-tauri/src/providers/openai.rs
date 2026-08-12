//! OpenAI Provider 适配器
//!
//! 官方 Administration/Organization Usage API（codereview P0 修复）：
//! - 用量：`GET {base}/organization/usage/completions?start_time=<unix>&end_time=<unix>&bucket_width=1d`
//! - 费用：`GET {base}/organization/costs?start_time=<unix>&end_time=<unix>&bucket_width=1d`
//!
//! 每个 day bucket 的 `results[]` 为多模型数组，需按桶聚合。
//! 统计口径（codereview P1 修复）：当日 = 最后一个 bucket 的 Token/费用；
//! `month_cost` = 近 30 天费用累计。`record_usage` 落库的即单日口径。
//!
//! 注意：该接口需要具备 Organization Admin 权限的 API Key。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;

pub struct OpenAIProvider;

// ---- 用量接口响应模型 ----
#[derive(Debug, Default, Deserialize)]
struct UsageResult {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    input_cached_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct UsageBucket {
    #[serde(default)]
    results: Vec<UsageResult>,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    #[serde(default)]
    data: Vec<UsageBucket>,
}

// ---- 费用接口响应模型 ----
#[derive(Debug, Default, Deserialize)]
struct CostAmount {
    #[serde(default)]
    value: f64,
}

#[derive(Debug, Deserialize)]
struct CostResult {
    #[serde(default)]
    amount: CostAmount,
}

#[derive(Debug, Deserialize)]
struct CostBucket {
    #[serde(default)]
    results: Vec<CostResult>,
}

#[derive(Debug, Deserialize)]
struct CostsResponse {
    #[serde(default)]
    data: Vec<CostBucket>,
}

#[async_trait]
impl ProviderAdapter for OpenAIProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        // 近 30 天（按天 bucket）
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(30);
        let base = config.api_url.trim_end_matches('/');

        let usage_url = format!(
            "{base}/organization/usage/completions?start_time={}&end_time={}&bucket_width=1d&limit=100",
            start.timestamp(),
            now.timestamp()
        );
        let costs_url = format!(
            "{base}/organization/costs?start_time={}&end_time={}&bucket_width=1d&limit=100",
            start.timestamp(),
            now.timestamp()
        );

        // 并行请求两个端点
        let (usage, costs) = tokio::join!(
            fetch_json::<UsageResponse>(&client, &usage_url, api_key),
            fetch_json::<CostsResponse>(&client, &costs_url, api_key),
        );
        let usage_data = usage?;
        let costs_data = costs?;

        // 聚合当日与 30 天累计
        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into();
        usage.total_tokens = usage_data
            .data
            .iter()
            .map(|b| b.results.iter().map(|r| r.total_tokens).sum::<u64>())
            .sum();
        if let Some(last) = usage_data.data.last() {
            usage.input_tokens = last.results.iter().map(|r| r.input_tokens).sum();
            usage.output_tokens = last.results.iter().map(|r| r.output_tokens).sum();
            usage.cached_tokens = last.results.iter().map(|r| r.input_cached_tokens).sum();
        }
        usage.today_cost = costs_data
            .data
            .last()
            .map(|b| b.results.iter().map(|r| r.amount.value).sum::<f64>());
        usage.month_cost = Some(
            costs_data
                .data
                .iter()
                .map(|b| b.results.iter().map(|r| r.amount.value).sum::<f64>())
                .sum(),
        );
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}

/// 发送 GET 请求并解析 JSON；非 2xx 返回带响应体的 Api 错误。
async fn fetch_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<T, ProviderError> {
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ProviderError::Api(format!("HTTP {status}: {body}")));
    }
    resp.json()
        .await
        .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_response_with_multi_model_results() {
        let json = r#"{
            "object": "list",
            "data": [
                {
                    "start_time": 1711929600,
                    "end_time": 1712016000,
                    "results": [
                        { "input_tokens": 100, "output_tokens": 200, "total_tokens": 300, "input_cached_tokens": 10 },
                        { "input_tokens": 50,  "output_tokens": 80,  "total_tokens": 130, "input_cached_tokens": 5 }
                    ]
                },
                {
                    "start_time": 1712016000,
                    "end_time": 1712102400,
                    "results": [
                        { "input_tokens": 1000, "output_tokens": 500, "total_tokens": 1500, "input_cached_tokens": 100 }
                    ]
                }
            ],
            "has_more": false
        }"#;
        let resp: UsageResponse = serde_json::from_str(json).expect("parse ok");
        assert_eq!(resp.data.len(), 2);
        // 当日 = 最后一个 bucket，跨模型累加
        let last = resp.data.last().unwrap();
        assert_eq!(last.results.len(), 1);
        assert_eq!(last.results[0].total_tokens, 1500);
        // 30 天总量 = 300+130+1500
        let total: u64 = resp
            .data
            .iter()
            .map(|b| b.results.iter().map(|r| r.total_tokens).sum::<u64>())
            .sum();
        assert_eq!(total, 1930);
    }

    #[test]
    fn parses_costs_response() {
        let json = r#"{
            "object": "list",
            "data": [
                {
                    "start_time": 1711929600,
                    "end_time": 1712016000,
                    "results": [
                        { "amount": { "value": 1.234, "currency": "usd" } },
                        { "amount": { "value": 0.5, "currency": "usd" } }
                    ]
                }
            ],
            "has_more": false
        }"#;
        let resp: CostsResponse = serde_json::from_str(json).expect("parse ok");
        let day_cost: f64 = resp.data[0].results.iter().map(|r| r.amount.value).sum();
        assert!((day_cost - 1.734).abs() < 1e-9);
    }
}
