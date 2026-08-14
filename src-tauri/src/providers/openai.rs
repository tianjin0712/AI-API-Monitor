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
#[derive(Debug, Default, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
struct UsageBucket {
    /// Unix 秒（bucket 起始）；用于按 UTC 日期精确筛"今日"。
    /// 兼容官方 v4 的 aggregation_timestamp 与部分响应/测试中的 start_time。
    #[serde(default, alias = "start_time")]
    aggregation_timestamp: Option<i64>,
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

// ---- 费用接口响应模型 ----
#[derive(Debug, Default, Clone, Deserialize)]
struct CostAmount {
    #[serde(default)]
    value: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct CostResult {
    #[serde(default)]
    amount: CostAmount,
}

#[derive(Debug, Clone, Deserialize)]
struct CostBucket {
    /// Unix 秒（bucket 起始）；用于按 UTC 日期精确筛"今日"。
    /// 兼容官方 v4 的 aggregation_timestamp 与部分响应/测试中的 start_time。
    #[serde(default, alias = "start_time")]
    aggregation_timestamp: Option<i64>,
    #[serde(default)]
    results: Vec<CostResult>,
}

#[derive(Debug, Deserialize)]
struct CostsResponse {
    #[serde(default)]
    data: Vec<CostBucket>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[async_trait]
impl ProviderAdapter for OpenAIProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = if config.provider_type == "custom" {
            crate::security::secure_http_client_for_custom_endpoint(&config.api_url, 20)
                .await
                .map_err(ProviderError::Http)?
        } else {
            crate::security::secure_http_client(20)
                .map_err(|e| ProviderError::Http(e.to_string()))?
        };

        // 近 30 天（按天 bucket）
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(30);
        let base = config.api_url.trim_end_matches('/');

        let usage_base = format!(
            "{base}/organization/usage/completions?start_time={}&end_time={}&bucket_width=1d&limit=100",
            start.timestamp(),
            now.timestamp()
        );
        let costs_base = format!(
            "{base}/organization/costs?start_time={}&end_time={}&bucket_width=1d&limit=100",
            start.timestamp(),
            now.timestamp()
        );

        // 并行请求两个端点（均带分页，P2）
        let (usage_data, costs_data) = tokio::join!(
            fetch_usage_pages(&client, &usage_base, api_key),
            fetch_costs_pages(&client, &costs_base, api_key),
        );
        let usage_data = usage_data?;
        let costs_data = costs_data?;

        // 今日口径（V0.5 复审）：按 bucket 的 UTC 日期精确筛今天，不再用 last()。
        // 分页可能把同一 bucket 拆成多条（limit=100），必须合并全部同日 bucket。
        let today = now.date_naive(); // P2：复用请求开始时捕获的 now，避免跨 UTC 午夜日期不一致
        let today_buckets: Vec<&UsageBucket> = usage_data
            .iter()
            .filter(|b| {
                b.aggregation_timestamp
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.date_naive() == today)
                    .unwrap_or(false)
            })
            .collect();
        let today_cost_buckets: Vec<&CostBucket> = costs_data
            .iter()
            .filter(|b| {
                b.aggregation_timestamp
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.date_naive() == today)
                    .unwrap_or(false)
            })
            .collect();

        // 聚合当日与 30 天累计
        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "$".into();
        usage.total_tokens = usage_data
            .iter()
            .map(|b| b.results.iter().map(|r| r.total_tokens).sum::<u64>())
            .sum();
        if !today_buckets.is_empty() {
            // 合并全部同日 bucket（P2：不再只取 first()）
            let (input, output, cached): (u64, u64, u64) =
                today_buckets.iter().fold((0, 0, 0), |(si, so, sc), b| {
                    (
                        si + b.results.iter().map(|r| r.input_tokens).sum::<u64>(),
                        so + b.results.iter().map(|r| r.output_tokens).sum::<u64>(),
                        sc + b.results.iter().map(|r| r.input_cached_tokens).sum::<u64>(),
                    )
                });
            usage.input_tokens = input;
            usage.output_tokens = output;
            usage.cached_tokens = cached;
            // 有今日 bucket 才写今日 Token（含真实 0）；无则保持 None（未知）
            usage.today_tokens = Some(input.saturating_add(output));
        }
        // 今日费用：按 UTC 日期筛（P1），今日无 bucket 时 None（未知）；
        // 负值/非有限值显式报错（P2），不污染趋势与预测
        usage.today_cost = if today_cost_buckets.is_empty() {
            None
        } else {
            let mut today = 0.0;
            for b in &today_cost_buckets {
                for r in &b.results {
                    today += validate_cost(r.amount.value)?;
                }
            }
            Some(today)
        };
        let mut month = 0.0;
        for b in &costs_data {
            for r in &b.results {
                month += validate_cost(r.amount.value)?;
            }
        }
        usage.month_cost = Some(month);
        usage.updated_at = now.to_rfc3339();
        Ok(usage)
    }
}

/// 校验费用值（P2）：负值/非有限值（NaN/Inf）显式报错，与前端趋势口径一致。
fn validate_cost(v: f64) -> Result<f64, ProviderError> {
    if !v.is_finite() || v < 0.0 {
        return Err(ProviderError::Api(format!(
            "OpenAI 费用非法（非有限或负值）: {v}"
        )));
    }
    Ok(v)
}

/// 分页拉取用量 buckets（has_more/next_page 循环）。
/// P1：与 Claude 分页器语义一致——completed 显式标记、重复 cursor 检测、cursor URL 编码、
/// 100 页上限触顶报错（不再 5 页静默截断）。
async fn fetch_usage_pages(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<UsageBucket>, ProviderError> {
    let mut all = Vec::new();
    let mut page: Option<String> = None;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut completed = false;
    for _ in 0..MAX_PAGES {
        let url = match &page {
            Some(p) => super::next_page_url(base_url, "page", p)?,
            None => base_url.to_string(),
        };
        let resp: UsageResponse = fetch_json(client, &url, api_key).await?;
        all.extend(resp.data);
        match super::advance_page(
            resp.has_more,
            resp.next_page.as_deref(),
            &mut seen,
            &mut page,
        )? {
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

/// 分页拉取费用 buckets（语义同上）。
async fn fetch_costs_pages(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<CostBucket>, ProviderError> {
    let mut all = Vec::new();
    let mut page: Option<String> = None;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut completed = false;
    for _ in 0..MAX_PAGES {
        let url = match &page {
            Some(p) => super::next_page_url(base_url, "page", p)?,
            None => base_url.to_string(),
        };
        let resp: CostsResponse = fetch_json(client, &url, api_key).await?;
        all.extend(resp.data);
        match super::advance_page(
            resp.has_more,
            resp.next_page.as_deref(),
            &mut seen,
            &mut page,
        )? {
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

/// 分页保护上限（与 Claude 分页器一致）。
const MAX_PAGES: usize = 100;

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

    #[test]
    fn bucket_parses_start_time_via_alias() {
        // 真实/测试响应可能用 start_time 而非 aggregation_timestamp（alias 双兼容）
        let json = r#"{"start_time": 1711929600, "end_time": 1712016000, "results": []}"#;
        let u: UsageBucket = serde_json::from_str(json).expect("usage parse ok");
        assert_eq!(u.aggregation_timestamp, Some(1711929600));
        let c: CostBucket = serde_json::from_str(json).expect("cost parse ok");
        assert_eq!(c.aggregation_timestamp, Some(1711929600));
    }

    #[test]
    fn validate_cost_rejects_negative_and_non_finite() {
        assert_eq!(super::validate_cost(1.5).unwrap(), 1.5);
        assert_eq!(super::validate_cost(0.0).unwrap(), 0.0);
        assert!(super::validate_cost(-1.0).is_err());
        assert!(super::validate_cost(f64::NAN).is_err());
        assert!(super::validate_cost(f64::INFINITY).is_err());
    }
}
