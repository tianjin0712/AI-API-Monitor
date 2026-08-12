//! Anthropic Claude Provider 适配器（组织级 Usage & Cost API）
//!
//! - 用量：`GET {base}/organizations/usage_report/messages?starting_at=...&ending_at=...&bucket_width=1d`
//! - 费用：`GET {base}/organizations/cost_report?starting_at=...&ending_at=...`
//! - 认证：`x-api-key: <Admin Key>` + `anthropic-version: 2023-06-01`（仅组织账户）
//! - 无余额查询端点（后付费账单）；成本单位为分（cents），换算美元需 ÷100
//!
//! 来源：https://platform.claude.com/docs/en/manage-claude/usage-cost-api

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct ClaudeProvider;

#[derive(Debug, Default, Clone, Deserialize)]
struct UsageResult {
    #[serde(default)]
    uncached_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<CacheCreation>,
    #[serde(default)]
    output_tokens: u64,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageBucket {
    #[serde(default)]
    results: Vec<UsageResult>,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    #[serde(default)]
    data: Vec<UsageBucket>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CostResult {
    #[serde(default)]
    amount: CostAmount,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct CostAmount {
    /// 成本（分，字符串），换算美元需 ÷100
    #[serde(default)]
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CostBucket {
    #[serde(default)]
    results: Vec<CostResult>,
}

#[derive(Debug, Deserialize)]
struct CostResponse {
    #[serde(default)]
    data: Vec<CostBucket>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[async_trait]
impl ProviderAdapter for ClaudeProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(30);
        let base = config.api_url.trim_end_matches('/');
        let start_s = start.to_rfc3339();
        let end_s = now.to_rfc3339();

        let usage_base = format!(
            "{base}/organizations/usage_report/messages?starting_at={start_s}&ending_at={end_s}&bucket_width=1d&limit=100"
        );
        let cost_base = format!(
            "{base}/organizations/cost_report?starting_at={start_s}&ending_at={end_s}"
        );

        let (usage_resp, cost_resp) = tokio::join!(
            fetch_with_pagination::<UsageResponse>(&client, &usage_base, api_key),
            fetch_with_pagination::<CostResponse>(&client, &cost_base, api_key),
        );
        let usage_data = usage_resp?;
        let cost_data = cost_resp?;

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into();

        // 用量：当日 = 最后 bucket；input 含未缓存/缓存读/缓存写
        usage.total_tokens = usage_data
            .iter()
            .map(bucket_input_output)
            .map(|(i, o)| i + o)
            .sum();
        if let Some(last) = usage_data.last() {
            let (input, output) = bucket_input_output(last);
            usage.input_tokens = input;
            usage.output_tokens = output;
            usage.cached_tokens = last
                .results
                .iter()
                .map(|r| r.cache_read_input_tokens)
                .sum();
        }
        // 费用：cents → USD（÷100）
        let month_cents: f64 = cost_data
            .iter()
            .map(|b| b.results.iter().map(|r| parse_cents(&r.amount.value)).sum::<f64>())
            .sum();
        usage.month_cost = Some(month_cents / 100.0);
        if let Some(last) = cost_data.last() {
            let day_cents: f64 = last
                .results
                .iter()
                .map(|r| parse_cents(&r.amount.value))
                .sum();
            usage.today_cost = Some(day_cents / 100.0);
        }
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}

/// 聚合一个 bucket 的 (input, output) token。
fn bucket_input_output(bucket: &UsageBucket) -> (u64, u64) {
    let input: u64 = bucket
        .results
        .iter()
        .map(|r| {
            r.uncached_input_tokens + r.cache_read_input_tokens + cache_write_tokens(r)
        })
        .sum();
    let output: u64 = bucket.results.iter().map(|r| r.output_tokens).sum();
    (input, output)
}

fn cache_write_tokens(r: &UsageResult) -> u64 {
    r.cache_creation
        .as_ref()
        .map(|c| c.ephemeral_5m_input_tokens + c.ephemeral_1h_input_tokens)
        .unwrap_or(0)
}

/// 解析成本字符串（分）；非数值视为 0。
fn parse_cents(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}

/// 带分页的请求（has_more/next_page，上限 5 页）。
async fn fetch_with_pagination<T>(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<T::Bucket>, ProviderError>
where
    T: PagedResponse,
{
    const MAX_PAGES: usize = 5;
    let mut all = Vec::new();
    let mut page: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let url = match &page {
            Some(p) => format!("{base_url}&page={p}"),
            None => base_url.to_string(),
        };
        let resp = fetch_json::<T>(client, &url, api_key).await?;
        all.extend(resp.buckets());
        match resp.next_page() {
            Some(next) if resp.has_more() && !next.is_empty() => page = Some(next.to_string()),
            _ => break,
        }
    }
    Ok(all)
}

/// 分页响应公共接口。
trait PagedResponse: serde::de::DeserializeOwned {
    type Bucket: Clone;
    fn buckets(&self) -> Vec<Self::Bucket>;
    fn has_more(&self) -> bool;
    fn next_page(&self) -> Option<&str>;
}

impl PagedResponse for UsageResponse {
    type Bucket = UsageBucket;
    fn buckets(&self) -> Vec<UsageBucket> {
        self.data.clone()
    }
    fn has_more(&self) -> bool {
        self.has_more
    }
    fn next_page(&self) -> Option<&str> {
        self.next_page.as_deref()
    }
}

impl PagedResponse for CostResponse {
    type Bucket = CostBucket;
    fn buckets(&self) -> Vec<CostBucket> {
        self.data.clone()
    }
    fn has_more(&self) -> bool {
        self.has_more
    }
    fn next_page(&self) -> Option<&str> {
        self.next_page.as_deref()
    }
}

/// 发送 GET 请求并解析 JSON；非 2xx 返回带响应体的 Api 错误。
async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
) -> Result<T, ProviderError> {
    let resp = client
        .get(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
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
    fn parses_usage_response() {
        let json = r#"{
            "data": [{
                "starting_at": "2025-08-01T00:00:00Z",
                "ending_at": "2025-08-02T00:00:00Z",
                "results": [{
                    "uncached_input_tokens": 1500,
                    "cache_read_input_tokens": 200,
                    "cache_creation": { "ephemeral_1h_input_tokens": 1000, "ephemeral_5m_input_tokens": 500 },
                    "output_tokens": 500,
                    "model": "claude-opus-4-6"
                }]
            }],
            "has_more": false
        }"#;
        let resp: UsageResponse = serde_json::from_str(json).expect("parse ok");
        let (input, output) = bucket_input_output(&resp.data[0]);
        assert_eq!(input, 1500 + 200 + 1500); // uncached + cache_read + cache_write
        assert_eq!(output, 500);
    }

    #[test]
    fn parses_cost_cents() {
        assert_eq!(parse_cents("123"), 123.0);
        assert_eq!(parse_cents("not-a-number"), 0.0);
        // 当日费用换算：cents → USD
        let day_cents = 234;
        assert!((day_cents as f64 / 100.0 - 2.34).abs() < 1e-9);
    }

    #[test]
    fn parses_paginated_flag() {
        let json = r#"{"data": [], "has_more": true, "next_page": "page_MjAyNS0wNS0xNFQwMDowMDowMFo="}"#;
        let resp: UsageResponse = serde_json::from_str(json).expect("parse ok");
        assert!(resp.has_more);
        assert!(resp.next_page.is_some());
    }
}
