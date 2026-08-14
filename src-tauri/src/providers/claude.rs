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
        let client = crate::security::secure_http_client(20)
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        self.fetch_usage_with_client(config, api_key, &client).await
    }
}

impl ClaudeProvider {
    /// 可注入 HTTP 客户端的查询实现（测试用 Mock 服务器；生产走安全客户端）。
    pub(crate) async fn fetch_usage_with_client(
        &self,
        config: &ProviderConfig,
        api_key: &str,
        client: &reqwest::Client,
    ) -> Result<ProviderUsage, ProviderError> {
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(30);
        let base = config.api_url.trim_end_matches('/');
        let start_s = start.to_rfc3339();
        let end_s = now.to_rfc3339();

        let usage_base = format!(
            "{base}/organizations/usage_report/messages?starting_at={start_s}&ending_at={end_s}&bucket_width=1d&limit=100"
        );
        let cost_base =
            // P1：Cost 显式按日 bucket 聚合，与 Usage 口径一致
            format!("{base}/organizations/cost_report?starting_at={start_s}&ending_at={end_s}&bucket_width=1d");

        let (usage_resp, cost_resp) = tokio::join!(
            fetch_with_pagination::<UsageResponse>(client, &usage_base, api_key),
            fetch_with_pagination::<CostResponse>(client, &cost_base, api_key),
        );
        let usage_data = usage_resp?;
        let cost_data = cost_resp?;

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into();

        // 今日口径（V0.4 复审 P1）：按 bucket 的 UTC 日期精确筛今天，不用 last()
        let today = now.date_naive(); // P2：复用请求开始时捕获的 now，避免跨 UTC 午夜日期不一致
        let today_usage: Vec<&UsageBucket> = usage_data
            .iter()
            .filter(|b| {
                bucket_date(b.starting_at.as_deref(), b.ending_at.as_deref())
                    .map(|d| d == today)
                    .unwrap_or(false)
            })
            .collect();
        let today_cost_buckets: Vec<&CostBucket> = cost_data
            .iter()
            .filter(|b| {
                bucket_date(b.starting_at.as_deref(), b.ending_at.as_deref())
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
        let (today_input, today_output) = today_usage
            .iter()
            .map(|b| bucket_input_output(b))
            .fold((0u64, 0u64), |(si, so), (i, o)| (si + i, so + o));
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
        let mut month_cents = 0.0;
        for b in &cost_data {
            for r in &b.results {
                month_cents += parse_cents(&r.amount.value)?;
            }
        }
        usage.month_cost = Some(month_cents / 100.0);
        // 今日费用：无今日 bucket 时 None（未知），不伪装成 Some(0)（P1）
        usage.today_cost = if today_cost_buckets.is_empty() {
            None
        } else {
            Some(day_cents(&today_cost_buckets)? / 100.0)
        };
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}

/// 聚合今日费用桶的 cents 总和（非法值经 ? 传播报错）。
fn day_cents(buckets: &[&CostBucket]) -> Result<f64, ProviderError> {
    let mut total = 0.0;
    for b in buckets {
        for r in &b.results {
            total += parse_cents(&r.amount.value)?;
        }
    }
    Ok(total)
}

/// 解析 RFC3339 时间戳为 UTC 日期。
fn bucket_day(ts: Option<&str>) -> Option<chrono::NaiveDate> {
    let ts = ts?;
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&chrono::Utc).date_naive())
        .ok()
}

/// bucket 所属日（P1）：优先 `starting_at`；仅当缺失时才回退 `ending_at`，
/// 且回退时取 `ending_at - 1ns` 的日期（避免把"今日 00:00 结束"的 bucket 归到明天）。
fn bucket_date(
    bucket_starting: Option<&str>,
    bucket_ending: Option<&str>,
) -> Option<chrono::NaiveDate> {
    if let Some(d) = bucket_day(bucket_starting) {
        return Some(d);
    }
    let ending = bucket_ending?;
    chrono::DateTime::parse_from_rfc3339(ending)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc) - chrono::Duration::nanoseconds(1))
        .map(|dt| dt.date_naive())
}

/// 聚合一个 bucket 的 (input, output) token。
fn bucket_input_output(bucket: &UsageBucket) -> (u64, u64) {
    let input: u64 = bucket
        .results
        .iter()
        .map(|r| r.uncached_input_tokens + r.cache_read_input_tokens + cache_write_tokens(r))
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

/// 解析成本字符串（分）。
/// P1：非法/负/非有限值显式报错，不再静默转 0 污染趋势与预测。
fn parse_cents(value: &str) -> Result<f64, ProviderError> {
    let v: f64 = value.trim().parse().map_err(|_| {
        ProviderError::Api(format!(
            "Claude 成本格式非法: {value:?}（可能接口协议变化）"
        ))
    })?;
    if !v.is_finite() || v < 0.0 {
        return Err(ProviderError::Api(format!(
            "Claude 成本非法（非有限或负值）: {value:?}"
        )));
    }
    Ok(v)
}

/// 带分页的请求（has_more/next_page）。
/// P0 修复：用显式 completed 标记判断是否触顶，不再依据 cursor 是否存在——
/// 否则正常翻页结束会被误判为"超过上限仍未结束"。
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
    let mut completed = false;
    for _ in 0..MAX_PAGES {
        let url = match &page {
            Some(p) => super::next_page_url(base_url, "page", p)?,
            None => base_url.to_string(),
        };
        let resp = fetch_json::<T>(client, &url, api_key).await?;
        all.extend(resp.buckets());
        match super::advance_page(resp.has_more(), resp.next_page(), &mut seen, &mut page)? {
            true => continue,
            false => {
                completed = true;
                break;
            }
        }
    }
    if !completed {
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
        return Err(ProviderError::Api(crate::security::safe_http_status_error(
            status,
        )));
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
        assert_eq!(parse_cents("123").unwrap(), 123.0);
        assert_eq!(parse_cents(" 45.5 ").unwrap(), 45.5);
        // 非法/负/非有限值必须显式报错（P1：不再静默转 0）
        assert!(parse_cents("not-a-number").is_err());
        assert!(parse_cents("nan").is_err());
        assert!(parse_cents("inf").is_err());
        assert!(parse_cents("-5").is_err());
        assert!(parse_cents("").is_err());
        // 今日费用换算：cents → USD
        let day_cents = 234;
        assert!((day_cents as f64 / 100.0 - 2.34).abs() < 1e-9);
    }

    #[test]
    fn parses_paginated_flag() {
        let json =
            r#"{"data": [], "has_more": true, "next_page": "page_MjAyNS0wNS0xNFQwMDowMDowMFo="}"#;
        let resp: UsageResponse = serde_json::from_str(json).expect("parse ok");
        assert!(resp.has_more);
        assert!(resp.next_page.is_some());
    }

    #[test]
    fn bucket_date_prefers_starting_at_and_ending_falls_back_minus_1ns() {
        // 标准日 bucket：start=8/12 00:00、end=8/13 00:00 → 归属 8/12（P1：不归到 8/13）
        let d = bucket_date(Some("2025-08-12T00:00:00Z"), Some("2025-08-13T00:00:00Z"));
        assert_eq!(d, chrono::NaiveDate::from_ymd_opt(2025, 8, 12));
        // 仅 starting_at
        let d2 = bucket_date(Some("2025-08-12T00:00:00Z"), None);
        assert_eq!(d2, chrono::NaiveDate::from_ymd_opt(2025, 8, 12));
        // 仅 ending_at：结束时刻减 1ns 归入前一秒所在日（8/13 00:00 → 8/12 23:59:59.999 → 8/12）
        let d3 = bucket_date(None, Some("2025-08-13T00:00:00Z"));
        assert_eq!(d3, chrono::NaiveDate::from_ymd_opt(2025, 8, 12));
        // 都无法解析
        assert_eq!(bucket_date(None, None), None);
    }
}
