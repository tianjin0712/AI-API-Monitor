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
    starting_at: Option<String>,
    #[serde(default)]
    ending_at: Option<String>,
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
    starting_at: Option<String>,
    #[serde(default)]
    ending_at: Option<String>,
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

        // 今日口径（V0.4 复审 P1）：按 bucket 的 UTC 日期精确筛今天，不用 last()
        let today = chrono::Utc::now().date_naive();
        let today_usage: Vec<&UsageBucket> = usage_data
            .iter()
            .filter(|b| {
                bucket_date(b.ending_at.as_deref(), b.starting_at.as_deref())
                    .map(|d| d == today)
                    .unwrap_or(false)
            })
            .collect();
        let today_cost_buckets: Vec<&CostBucket> = cost_data
            .iter()
            .filter(|b| {
                bucket_date(b.ending_at.as_deref(), b.starting_at.as_deref())
                    .map(|d| d == today)
                    .unwrap_or(false)
            })
            .collect();

        // 近 30 天总量
        usage.total_tokens = usage_data
            .iter()
            .map(bucket_input_output)
            .map(|(i, o)| i + o)
            .sum();
        // 今日用量（无今天 bucket 时保持 0）
        let (today_input, today_output) = today_usage.iter().map(|b| bucket_input_output(b)).fold(
            (0u64, 0u64),
            |(si, so), (i, o)| (si + i, so + o),
        );
        usage.input_tokens = today_input;
        usage.output_tokens = today_output;
        usage.cached_tokens = today_usage
            .iter()
            .flat_map(|b| b.results.iter())
            .map(|r| r.cache_read_input_tokens)
            .sum();
        // 有今日 bucket 才写今日 Token（含真实 0）；无则保持 None（未知）
        if !today_usage.is_empty() {
            usage.today_tokens = Some(today_input.saturating_add(today_output));
        }
        // 费用：cents → USD（÷100）；今日与近 30 天
        let month_cents: f64 = cost_data
            .iter()
            .map(|b| b.results.iter().map(|r| parse_cents(&r.amount.value)).sum::<f64>())
            .sum();
        usage.month_cost = Some(month_cents / 100.0);
        // 今日费用：无今日 bucket 时 None（未知），不伪装成 Some(0)（P1）
        usage.today_cost = if today_cost_buckets.is_empty() {
            None
        } else {
            Some(day_cents(&today_cost_buckets) / 100.0)
        };
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}

/// 聚合今日费用桶的 cents 总和（纯函数，便于测试）。
fn day_cents(buckets: &[&CostBucket]) -> f64 {
    buckets
        .iter()
        .flat_map(|b| b.results.iter())
        .map(|r| parse_cents(&r.amount.value))
        .sum()
}

/// 解析 bucket 的 UTC 日期（优先 ending_at，回退 starting_at）；无法解析返回 None。
fn bucket_day(ts: Option<&str>) -> Option<chrono::NaiveDate> {
    let ts = ts?;
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&chrono::Utc).date_naive())
        .ok()
}

/// bucket 的日期：优先 ending_at，缺失时回退 starting_at。
fn bucket_date(bucket_ending: Option<&str>, bucket_starting: Option<&str>) -> Option<chrono::NaiveDate> {
    bucket_day(bucket_ending).or_else(|| bucket_day(bucket_starting))
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

/// 带分页的请求（has_more/next_page）。
/// V0.4 复审 P1：不设 5 页截断——循环至 has_more=false；检测空/重复 next_page 防死循环；
/// 设高上限（100 页）防失控，触顶返回显式错误（不把截断数据当完整结果）。
async fn fetch_with_pagination<T>(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<T::Bucket>, ProviderError>
where
    T: PagedResponse,
{
    const MAX_PAGES: usize = 100;
    let mut all = Vec::new();
    let mut page: Option<String> = None;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..MAX_PAGES {
        let url = match &page {
            Some(p) => format!("{base_url}&page={p}"),
            None => base_url.to_string(),
        };
        let resp = fetch_json::<T>(client, &url, api_key).await?;
        all.extend(resp.buckets());
        match resp.next_page() {
            Some(next) if resp.has_more() && !next.is_empty() => {
                // 重复 next_page：服务端异常，避免死循环
                if !seen.insert(next.to_string()) {
                    return Err(ProviderError::Api(format!(
                        "分页游标重复（服务端异常）: {next}"
                    )));
                }
                page = Some(next.to_string());
            }
            Some(_) | None => break, // has_more=false 或 next_page 缺失：正常结束
        }
    }
    // 触顶仍 has_more：数据不完整，显式报错而非静默截断
    if page.is_some() {
        return Err(ProviderError::Api(format!(
            "分页超过 {MAX_PAGES} 页仍未结束，数据可能不完整"
        )));
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

    #[test]
    fn bucket_date_prefers_ending_at_and_falls_back() {
        // ending_at 优先
        let d = bucket_date(Some("2025-08-13T00:00:00Z"), Some("2025-08-12T00:00:00Z"));
        assert_eq!(d, chrono::NaiveDate::from_ymd_opt(2025, 8, 13));
        // 缺失 ending_at 回退 starting_at
        let d2 = bucket_date(None, Some("2025-08-12T00:00:00Z"));
        assert_eq!(d2, chrono::NaiveDate::from_ymd_opt(2025, 8, 12));
        // 都无法解析
        assert_eq!(bucket_date(None, None), None);
    }
}
