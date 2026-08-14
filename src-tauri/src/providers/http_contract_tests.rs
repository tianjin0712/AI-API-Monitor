//! Provider HTTP 层 Mock 合约测试（第四轮：HTTP 层 Mock 基础设施落地）。
//!
//! 通过本地脚本化 Mock 服务器（[`test_http`]）+ 可注入测试客户端
//! （`crate::security::insecure_test_http_client`），驱动各 Provider 适配器的
//! `fetch_usage_with_client` 走完「构造请求 → 发送 → 状态码处理 → JSON 解析 →
//! 业务校验 → 聚合」全链路，覆盖 TODO_LIST 第四节的合约矩阵：
//! 成功响应、401/403、429、5xx、非 JSON、超时、网络中断、分页。
//!
//! 生产安全策略（HTTPS-only、私网拒绝、DNS 固定）在 security.rs 的
//! 独立测试中验证，此处只验证 Provider 适配器在合法传输层上的行为。

use super::claude;
use super::deepseek;
use super::openai;
use super::openrouter;
use super::siliconflow;
use super::test_http::{MockResponse, MockServer};
use super::{ProviderConfig, ProviderError, ProviderUsage};
use crate::security::insecure_test_http_client;
use std::time::Duration;

// ---------------------------------------------------------------------------
// 公共辅助
// ---------------------------------------------------------------------------

fn provider_config(base: &str, provider_type: &str) -> ProviderConfig {
    ProviderConfig {
        id: 1,
        name: "mock".into(),
        provider_type: provider_type.into(),
        api_url: base.into(),
        key_ref: "keyring:key_mock".into(),
        key_hint: "sk-****1234".into(),
        enabled: true,
        created_time: String::new(),
        updated_time: String::new(),
    }
}

fn test_client(timeout_secs: u64) -> reqwest::Client {
    insecure_test_http_client(timeout_secs).expect("build test client")
}

fn is_http_error(result: &Result<ProviderUsage, ProviderError>) -> bool {
    matches!(result, Err(ProviderError::Http(_)))
}

fn api_error_contains(result: &Result<ProviderUsage, ProviderError>, fragment: &str) -> bool {
    matches!(result, Err(ProviderError::Api(message)) if message.contains(fragment))
}

fn today_utc_midnight_ts() -> i64 {
    chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight")
        .and_utc()
        .timestamp()
}

/// 今日与明日 UTC 日期（YYYY-MM-DD），用于构造命中“今日口径”的 bucket。
fn today_and_tomorrow() -> (String, String) {
    let today = chrono::Utc::now().date_naive();
    (
        today.to_string(),
        (today + chrono::Days::new(1)).to_string(),
    )
}

// ---------------------------------------------------------------------------
// DeepSeek
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deepseek_success_parses_cny_balance() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00"}]}"#,
    )]);
    let config = provider_config(&server.base_url(), "deepseek");
    let usage = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await
        .expect("fetch ok");
    assert_eq!(usage.balance, Some(110.0));
    assert_eq!(usage.currency, "CNY");
    assert!(
        server
            .request_lines()
            .iter()
            .any(|line| line.starts_with("GET /user/balance ")),
        "应请求 /user/balance: {:?}",
        server.request_lines()
    );
}

#[tokio::test]
async fn deepseek_unavailable_account_is_explicit_error() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"is_available":false,"balance_infos":[]}"#,
    )]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "账户不可用"));
}

#[tokio::test]
async fn deepseek_unauthorized_is_auth_error() {
    let server = MockServer::start(vec![MockResponse::json(401, r#"{"error":"bad key"}"#)]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn deepseek_forbidden_is_auth_error() {
    let server = MockServer::start(vec![MockResponse::json(403, "{}")]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn deepseek_rate_limited_is_explicit() {
    let server = MockServer::start(vec![MockResponse::json(429, "{}")]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "rate limited"), "{result:?}");
}

#[tokio::test]
async fn deepseek_server_error_is_provider_unavailable() {
    for status in [500u16, 502, 503] {
        let server = MockServer::start(vec![MockResponse::json(status, "{}")]);
        let config = provider_config(&server.base_url(), "deepseek");
        let result = deepseek::DeepSeekProvider
            .fetch_usage_with_client(&config, "sk-test", &test_client(5))
            .await;
        assert!(
            api_error_contains(&result, "provider unavailable"),
            "status {status}: {result:?}"
        );
    }
}

#[tokio::test]
async fn deepseek_non_json_body_is_parse_error() {
    let server = MockServer::start(vec![MockResponse::json(200, "<html>gateway error</html>")]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "响应解析失败"), "{result:?}");
}

#[tokio::test]
async fn deepseek_timeout_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "{}").with_delay(Duration::from_secs(6))
    ]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(2))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

#[tokio::test]
async fn deepseek_connection_drop_is_http_error() {
    let server = MockServer::start(vec![MockResponse::drop_connection()]);
    let config = provider_config(&server.base_url(), "deepseek");
    let result = deepseek::DeepSeekProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

// ---------------------------------------------------------------------------
// OpenRouter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn openrouter_success_parses_limits() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"data":{"limit_remaining":12.5,"limit_reset":"2026-08-15T00:00:00Z","usage_daily":1.25,"usage_monthly":30.0}}"#,
    )]);
    let config = provider_config(&server.base_url(), "openrouter");
    let usage = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await
        .expect("fetch ok");
    assert_eq!(usage.balance, Some(12.5));
    assert_eq!(usage.today_cost, Some(1.25));
    assert_eq!(usage.month_cost, Some(30.0));
    assert_eq!(usage.reset_time.as_deref(), Some("2026-08-15T00:00:00Z"));
    assert_eq!(usage.currency, "$");
    assert!(
        server
            .request_lines()
            .iter()
            .any(|line| line.contains("/api/v1/key")),
        "应请求 /api/v1/key: {:?}",
        server.request_lines()
    );
}

#[tokio::test]
async fn openrouter_unauthorized_is_auth_error() {
    let server = MockServer::start(vec![MockResponse::json(401, "{}")]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn openrouter_rate_limited_is_explicit() {
    let server = MockServer::start(vec![MockResponse::json(429, "{}")]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "rate limited"), "{result:?}");
}

#[tokio::test]
async fn openrouter_server_error_is_provider_unavailable() {
    let server = MockServer::start(vec![MockResponse::json(500, "{}")]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "provider unavailable"),
        "{result:?}"
    );
}

#[tokio::test]
async fn openrouter_non_json_body_is_parse_error() {
    let server = MockServer::start(vec![MockResponse::json(200, "not json at all")]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "响应解析失败"), "{result:?}");
}

#[tokio::test]
async fn openrouter_timeout_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "{}").with_delay(Duration::from_secs(6))
    ]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(2))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

#[tokio::test]
async fn openrouter_connection_drop_is_http_error() {
    let server = MockServer::start(vec![MockResponse::drop_connection()]);
    let config = provider_config(&server.base_url(), "openrouter");
    let result = openrouter::OpenRouterProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

// ---------------------------------------------------------------------------
// SiliconFlow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn siliconflow_success_parses_balance() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"code":0,"message":"ok","data":{"balance":"19.00"}}"#,
    )]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let usage = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await
        .expect("fetch ok");
    assert_eq!(usage.balance, Some(19.0));
    assert_eq!(usage.currency, "¥");
}

#[tokio::test]
async fn siliconflow_missing_balance_is_explicit_error() {
    // TODO_LIST 已确认行为：余额缺失不允许写入虚假零值
    let server = MockServer::start(vec![MockResponse::json(200, r#"{"code":0,"data":{}}"#)]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "缺少余额字段"), "{result:?}");
}

#[tokio::test]
async fn siliconflow_business_error_is_reported() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"code":1002,"message":"余额不足"}"#,
    )]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "余额不足"), "{result:?}");
}

#[tokio::test]
async fn siliconflow_missing_data_is_explicit_error() {
    let server = MockServer::start(vec![MockResponse::json(200, r#"{"code":0}"#)]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "缺少 data 字段"), "{result:?}");
}

#[tokio::test]
async fn siliconflow_unauthorized_is_auth_error() {
    let server = MockServer::start(vec![MockResponse::json(401, "{}")]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn siliconflow_rate_limited_is_explicit() {
    let server = MockServer::start(vec![MockResponse::json(429, "{}")]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "rate limited"), "{result:?}");
}

#[tokio::test]
async fn siliconflow_server_error_is_provider_unavailable() {
    let server = MockServer::start(vec![MockResponse::json(503, "{}")]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "provider unavailable"),
        "{result:?}"
    );
}

#[tokio::test]
async fn siliconflow_non_json_body_is_parse_error() {
    let server = MockServer::start(vec![MockResponse::json(200, "<html>502</html>")]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "响应解析失败"), "{result:?}");
}

#[tokio::test]
async fn siliconflow_timeout_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "{}").with_delay(Duration::from_secs(6))
    ]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(2))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

#[tokio::test]
async fn siliconflow_connection_drop_is_http_error() {
    let server = MockServer::start(vec![MockResponse::drop_connection()]);
    let config = provider_config(&server.base_url(), "siliconflow");
    let result = siliconflow::SiliconFlowProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

// ---------------------------------------------------------------------------
// Claude（双端点并发 + 分页）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn claude_success_aggregates_today_buckets() {
    let (today, tomorrow) = today_and_tomorrow();
    let usage_json = format!(
        r#"{{"data":[{{"starting_at":"{today}T00:00:00Z","ending_at":"{tomorrow}T00:00:00Z","results":[{{"uncached_input_tokens":1000,"cache_read_input_tokens":100,"output_tokens":500}}]}}],"has_more":false}}"#
    );
    let cost_json = format!(
        r#"{{"data":[{{"starting_at":"{today}T00:00:00Z","ending_at":"{tomorrow}T00:00:00Z","results":[{{"amount":{{"value":"200"}}}}]}}],"has_more":false}}"#
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, usage_json).for_path("usage_report"),
        MockResponse::json(200, cost_json).for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let usage = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(10))
        .await
        .expect("fetch ok");
    assert_eq!(usage.total_tokens, 1600);
    assert_eq!(usage.input_tokens, 1100);
    assert_eq!(usage.output_tokens, 500);
    assert_eq!(usage.cached_tokens, 100);
    assert_eq!(usage.today_tokens, Some(1600));
    assert_eq!(usage.today_cost, Some(2.0), "200 分 = 2.00 USD");
    assert_eq!(usage.month_cost, Some(2.0));
    assert_eq!(usage.currency, "$");
}

#[tokio::test]
async fn claude_pagination_aggregates_all_pages() {
    let (today, tomorrow) = today_and_tomorrow();
    let page1 = format!(
        r#"{{"data":[{{"starting_at":"{today}T00:00:00Z","ending_at":"{tomorrow}T00:00:00Z","results":[{{"uncached_input_tokens":100,"output_tokens":50}}]}}],"has_more":true,"next_page":"c2"}}"#
    );
    let page2 = format!(
        r#"{{"data":[{{"starting_at":"{today}T00:00:00Z","ending_at":"{tomorrow}T00:00:00Z","results":[{{"uncached_input_tokens":200,"output_tokens":100}}]}}],"has_more":false}}"#
    );
    let cost_json = format!(
        r#"{{"data":[{{"starting_at":"{today}T00:00:00Z","ending_at":"{tomorrow}T00:00:00Z","results":[{{"amount":{{"value":"100"}}}}]}}],"has_more":false}}"#
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, page1).for_path("usage_report"),
        MockResponse::json(200, page2).for_path("page=c2"),
        MockResponse::json(200, cost_json).for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let usage = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(10))
        .await
        .expect("fetch ok");
    assert_eq!(usage.total_tokens, 450, "两页合计 (100+50)+(200+100)");
    assert_eq!(usage.today_tokens, Some(450));
    assert_eq!(usage.month_cost, Some(1.0));
    assert_eq!(usage.today_cost, Some(1.0));
}

#[tokio::test]
async fn claude_unauthorized_is_auth_error() {
    let server = MockServer::start(vec![
        MockResponse::json(401, "{}").for_path("usage_report"),
        MockResponse::json(401, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn claude_rate_limited_is_explicit() {
    let server = MockServer::start(vec![
        MockResponse::json(429, "{}").for_path("usage_report"),
        MockResponse::json(429, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "rate limited"), "{result:?}");
}

#[tokio::test]
async fn claude_server_error_is_provider_unavailable() {
    let server = MockServer::start(vec![
        MockResponse::json(500, "{}").for_path("usage_report"),
        MockResponse::json(200, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "provider unavailable"),
        "{result:?}"
    );
}

#[tokio::test]
async fn claude_non_json_usage_is_parse_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "oops").for_path("usage_report"),
        MockResponse::json(200, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "响应解析失败"), "{result:?}");
}

#[tokio::test]
async fn claude_timeout_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "{}")
            .for_path("usage_report")
            .with_delay(Duration::from_secs(6)),
        MockResponse::json(200, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(2))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

#[tokio::test]
async fn claude_connection_drop_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::drop_connection().for_path("usage_report"),
        MockResponse::json(200, "{}").for_path("cost_report"),
    ]);
    let config = provider_config(&server.base_url(), "claude");
    let result = claude::ClaudeProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

// ---------------------------------------------------------------------------
// OpenAI（双端点并发 + 分页）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn openai_success_aggregates_today_buckets() {
    let ts = today_utc_midnight_ts();
    let usage_json = format!(
        r#"{{"data":[{{"aggregation_timestamp":{ts},"results":[{{"input_tokens":100,"output_tokens":50,"total_tokens":150,"input_cached_tokens":10}}]}}],"has_more":false}}"#
    );
    let cost_json = format!(
        r#"{{"data":[{{"aggregation_timestamp":{ts},"results":[{{"amount":{{"value":2.5}}}}]}}],"has_more":false}}"#
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, usage_json).for_path("usage"),
        MockResponse::json(200, cost_json).for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let usage = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(10))
        .await
        .expect("fetch ok");
    assert_eq!(usage.total_tokens, 150);
    assert_eq!(usage.input_tokens, 100);
    assert_eq!(usage.output_tokens, 50);
    assert_eq!(usage.cached_tokens, 10);
    assert_eq!(usage.today_tokens, Some(150));
    assert_eq!(usage.today_cost, Some(2.5));
    assert_eq!(usage.month_cost, Some(2.5));
    assert_eq!(usage.currency, "$");
}

#[tokio::test]
async fn openai_pagination_aggregates_all_pages() {
    let ts = today_utc_midnight_ts();
    let page1 = format!(
        r#"{{"data":[{{"aggregation_timestamp":{ts},"results":[{{"input_tokens":60,"output_tokens":40,"total_tokens":100}}]}}],"has_more":true,"next_page":"n2"}}"#
    );
    let page2 = format!(
        r#"{{"data":[{{"aggregation_timestamp":{ts},"results":[{{"input_tokens":30,"output_tokens":20,"total_tokens":50}}]}}],"has_more":false}}"#
    );
    let cost_json = format!(
        r#"{{"data":[{{"aggregation_timestamp":{ts},"results":[{{"amount":{{"value":1.0}}}}]}}],"has_more":false}}"#
    );
    let server = MockServer::start(vec![
        MockResponse::json(200, page1).for_path("usage"),
        MockResponse::json(200, page2).for_path("page=n2"),
        MockResponse::json(200, cost_json).for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let usage = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(10))
        .await
        .expect("fetch ok");
    assert_eq!(usage.total_tokens, 150, "两页合计 100+50");
    assert_eq!(usage.today_tokens, Some(150));
    assert_eq!(usage.today_cost, Some(1.0));
    assert_eq!(usage.month_cost, Some(1.0));
}

#[tokio::test]
async fn openai_unauthorized_is_auth_error() {
    let server = MockServer::start(vec![
        MockResponse::json(401, "{}").for_path("usage"),
        MockResponse::json(401, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn openai_forbidden_is_auth_error() {
    let server = MockServer::start(vec![
        MockResponse::json(403, "{}").for_path("usage"),
        MockResponse::json(403, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "authentication failed"),
        "{result:?}"
    );
}

#[tokio::test]
async fn openai_rate_limited_is_explicit() {
    let server = MockServer::start(vec![
        MockResponse::json(429, "{}").for_path("usage"),
        MockResponse::json(429, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "rate limited"), "{result:?}");
}

#[tokio::test]
async fn openai_server_error_is_provider_unavailable() {
    let server = MockServer::start(vec![
        MockResponse::json(500, "{}").for_path("usage"),
        MockResponse::json(200, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(
        api_error_contains(&result, "provider unavailable"),
        "{result:?}"
    );
}

#[tokio::test]
async fn openai_non_json_usage_is_parse_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "broken").for_path("usage"),
        MockResponse::json(200, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(api_error_contains(&result, "响应解析失败"), "{result:?}");
}

#[tokio::test]
async fn openai_timeout_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::json(200, "{}")
            .for_path("usage")
            .with_delay(Duration::from_secs(6)),
        MockResponse::json(200, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(2))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}

#[tokio::test]
async fn openai_connection_drop_is_http_error() {
    let server = MockServer::start(vec![
        MockResponse::drop_connection().for_path("usage"),
        MockResponse::json(200, "{}").for_path("costs"),
    ]);
    let config = provider_config(&server.base_url(), "openai");
    let result = openai::OpenAIProvider
        .fetch_usage_with_client(&config, "sk-test", &test_client(5))
        .await;
    assert!(is_http_error(&result), "{result:?}");
}
