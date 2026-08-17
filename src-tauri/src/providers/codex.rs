//! Codex quota adapter using the official runtime bundled with Codex Desktop.
//!
//! Authentication remains entirely owned by the official runtime. This module
//! never opens Codex Home, browser data, cookies, `auth.json`, or token stores.

use super::desktop_runtime::{DesktopRuntimeResolver, RuntimeSource};
use super::{
    CodexRateLimitWindow, CodexUsageDetails, ProviderAdapter, ProviderConfig, ProviderError,
    ProviderUsage,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::{thread, time::Duration};
use tauri::Emitter;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeStatus {
    pub installed: bool,
    pub logged_in: bool,
    pub runtime_source: Option<String>,
}

pub struct CodexProvider;
pub const DEFAULT_CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";

/// Builds a Codex runtime command without allowing Windows to create a visible
/// console window. Redirecting stdio alone does not suppress a console for a
/// GUI parent process.
fn codex_command(executable: &std::path::Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        // CREATE_NO_WINDOW: keep periodic `login status` and `app-server`
        // invocations from flashing a terminal for installed users.
        command.creation_flags(0x0800_0000);
    }
    command
}

fn write_message(child: &mut Child, message: Value) -> Result<(), ProviderError> {
    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| ProviderError::Api("Codex Desktop Runtime 标准输入不可用".into()))?;
    writeln!(stdin, "{message}")
        .and_then(|_| stdin.flush())
        .map_err(|_| ProviderError::Api("无法向 Codex Desktop Runtime 发送请求".into()))
}

fn read_response(
    reader: &mut BufReader<impl std::io::Read>,
    id: i64,
) -> Result<Value, ProviderError> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|_| ProviderError::Api("读取 Codex Desktop Runtime 响应失败".into()))?
            == 0
        {
            return Err(ProviderError::Api("Codex Desktop Runtime 提前退出".into()));
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("id").and_then(Value::as_i64) != Some(id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            let code = error.get("code").and_then(Value::as_i64);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("请求被拒绝");
            return Err(ProviderError::Api(match code {
                Some(code) => format!("Codex Runtime 错误 {code}: {message}"),
                None => format!("Codex Runtime 错误: {message}"),
            }));
        }
        return value
            .get("result")
            .cloned()
            .ok_or_else(|| ProviderError::Api("Codex 额度响应缺少结果".into()));
    }
}

fn runtime_source_label(source: RuntimeSource) -> &'static str {
    match source {
        RuntimeSource::DesktopUserRuntime => "desktop-user-runtime",
        RuntimeSource::DesktopInstall => "desktop-install",
        RuntimeSource::PackagedDesktop => "desktop-package",
        RuntimeSource::StandaloneCli => "standalone-cli",
    }
}

pub fn runtime_status() -> CodexRuntimeStatus {
    let candidates = DesktopRuntimeResolver::from_environment().resolve_candidates();
    for runtime in &candidates {
        let logged_in = codex_command(&runtime.executable)
            .args(["login", "status"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if logged_in {
            return CodexRuntimeStatus {
                installed: true,
                logged_in: true,
                runtime_source: Some(runtime_source_label(runtime.source).to_owned()),
            };
        }
    }
    CodexRuntimeStatus {
        installed: !candidates.is_empty(),
        logged_in: false,
        runtime_source: candidates
            .first()
            .map(|runtime| runtime_source_label(runtime.source).to_owned()),
    }
}

pub fn start_login() -> Result<(), ProviderError> {
    let runtime = DesktopRuntimeResolver::from_environment()
        .resolve_candidates()
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Api("未安装 ChatGPT/Codex Desktop 或 Codex CLI".into()))?;
    codex_command(&runtime.executable)
        .arg("login")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| ProviderError::Api("无法启动 Codex 官方登录流程".into()))
}

/// Keeps a lightweight official App Server connection for push quota updates.
/// The existing Dashboard scheduler remains the low-frequency fallback.
pub fn start_rate_limit_monitor(app: tauri::AppHandle) {
    thread::spawn(move || {
        let mut backoff_secs = 5_u64;
        loop {
            let candidates = DesktopRuntimeResolver::from_environment().resolve_candidates();
            let mut connected = false;
            for runtime in candidates {
                let Ok(mut child) = codex_command(&runtime.executable)
                    .arg("app-server")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                else {
                    continue;
                };
                let Some(stdout) = child.stdout.take() else {
                    let _ = child.kill();
                    continue;
                };
                let mut reader = BufReader::new(stdout);
                if write_message(&mut child, json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"ai-api-monitor","version":env!("CARGO_PKG_VERSION")}}})).is_err()
                    || read_response(&mut reader, 1).is_err()
                    || write_message(&mut child, json!({"method":"initialized"})).is_err()
                {
                    let _ = child.kill();
                    continue;
                }
                connected = true;
                backoff_secs = 5;
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    let Ok(message) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    if message.get("method").and_then(Value::as_str)
                        != Some("account/rateLimits/updated")
                    {
                        continue;
                    }
                    let payload = message.get("params").unwrap_or(&message);
                    if let Ok(usage) = parse_rate_limits(payload, runtime.source) {
                        let _ = app.emit("codex-rate-limits-updated", usage);
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_secs(backoff_secs));
            if !connected {
                backoff_secs = (backoff_secs * 2).min(300);
            }
        }
    });
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| value.as_f64().or_else(|| value.as_u64().map(|v| v as f64)))
}

fn integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value.as_u64().or_else(|| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && *number >= 0.0)
                .map(|number| number as u64)
        })
    })
}

fn parse_window(
    snapshot: &Value,
    window_kind: &str,
    limit_id: Option<String>,
    limit_name: Option<String>,
) -> Option<CodexRateLimitWindow> {
    let window = snapshot.get(window_kind)?;
    let used_percent = number(window.get("usedPercent"))?;
    // Only accept explicit token fields from the same rate-limit window. The
    // lifetime/daily Usage values are not quota limits and must not be used here.
    let token_limit = ["tokenLimit", "tokensLimit", "inputTokenLimit"]
        .iter()
        .find_map(|key| integer(window.get(*key)));
    let tokens_used = ["tokensUsed", "usedTokens", "tokenUsage"]
        .iter()
        .find_map(|key| integer(window.get(*key)));
    let tokens_remaining = token_limit
        .zip(tokens_used)
        .map(|(limit, used)| limit.saturating_sub(used));
    Some(CodexRateLimitWindow {
        limit_id,
        limit_name,
        window_kind: window_kind.to_owned(),
        used_percent,
        remaining_percent: (100.0 - used_percent).clamp(0.0, 100.0),
        window_duration_mins: window.get("windowDurationMins").and_then(Value::as_u64),
        resets_at: window.get("resetsAt").and_then(Value::as_i64),
        unlimited: window
            .get("unlimited")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        token_limit,
        tokens_used,
        tokens_remaining,
    })
}

fn push_snapshot(windows: &mut Vec<CodexRateLimitWindow>, snapshot: &Value) {
    let limit_id = snapshot
        .get("limitId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let limit_name = snapshot
        .get("limitName")
        .and_then(Value::as_str)
        .map(str::to_owned);
    for kind in ["primary", "secondary"] {
        if let Some(window) = parse_window(snapshot, kind, limit_id.clone(), limit_name.clone()) {
            let duplicate = windows.iter().any(|existing| {
                existing.limit_id == window.limit_id
                    && existing.window_kind == window.window_kind
                    && existing.window_duration_mins == window.window_duration_mins
            });
            if !duplicate {
                windows.push(window);
            }
        }
    }
}

fn parse_rate_limits(
    response: &Value,
    source: RuntimeSource,
) -> Result<ProviderUsage, ProviderError> {
    let snapshot = response.get("rateLimits").unwrap_or(response);
    let mut windows = Vec::new();
    if snapshot.is_object() {
        push_snapshot(&mut windows, snapshot);
    }
    if let Some(by_id) = response.get("rateLimitsByLimitId") {
        if let Some(items) = by_id.as_object() {
            for item in items.values() {
                push_snapshot(&mut windows, item);
            }
        } else if let Some(items) = by_id.as_array() {
            for item in items {
                push_snapshot(&mut windows, item);
            }
        }
    }
    let plan_type = snapshot
        .get("planType")
        .or_else(|| response.get("planType"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let credits = snapshot
        .get("credits")
        .or_else(|| response.get("rateLimitResetCredits"))
        .filter(|value| !value.is_null())
        .cloned();

    if windows.is_empty() && credits.is_none() {
        return Err(ProviderError::Api("当前账户暂无 Codex 额度信息".into()));
    }
    let mut usage = ProviderUsage::empty("codex");
    usage.remaining = windows
        .iter()
        .filter(|window| !window.unlimited)
        .map(|window| window.remaining_percent)
        .reduce(f64::min);
    usage.reset_time = windows
        .iter()
        .filter_map(|window| window.resets_at)
        .min()
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|value| value.to_rfc3339());
    usage.codex = Some(CodexUsageDetails {
        runtime_source: runtime_source_label(source).to_owned(),
        plan_type,
        credits,
        windows,
    });
    usage.updated_at = chrono::Utc::now().to_rfc3339();
    Ok(usage)
}

fn apply_account_usage(usage: &mut ProviderUsage, response: &Value) {
    usage.total_tokens = response
        .get("summary")
        .and_then(|summary| summary.get("lifetimeTokens"))
        .and_then(Value::as_u64)
        .unwrap_or(usage.total_tokens);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    usage.today_tokens = response
        .get("dailyUsageBuckets")
        .and_then(Value::as_array)
        .and_then(|buckets| {
            buckets.iter().find(|bucket| {
                bucket.get("startDate").and_then(Value::as_str) == Some(today.as_str())
            })
        })
        .and_then(|bucket| bucket.get("tokens"))
        .and_then(Value::as_u64);
}

fn fetch_from_runtime(
    runtime: &super::desktop_runtime::ResolvedRuntime,
) -> Result<ProviderUsage, ProviderError> {
    let mut child = codex_command(&runtime.executable)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ProviderError::Api("无法启动 Codex Runtime".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProviderError::Api("Codex Runtime 标准输出不可用".into()))?;
    let mut reader = BufReader::new(stdout);
    let result = (|| {
        write_message(
            &mut child,
            json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"ai-api-monitor","version":env!("CARGO_PKG_VERSION")}}}),
        )?;
        let _ = read_response(&mut reader, 1)?;
        write_message(&mut child, json!({"method":"initialized"}))?;
        write_message(
            &mut child,
            json!({"id":2,"method":"account/rateLimits/read","params":null}),
        )?;
        let mut usage = parse_rate_limits(&read_response(&mut reader, 2)?, runtime.source)?;
        write_message(
            &mut child,
            json!({"id":3,"method":"account/usage/read","params":null}),
        )?;
        if let Ok(account_usage) = read_response(&mut reader, 3) {
            apply_account_usage(&mut usage, &account_usage);
        }
        Ok(usage)
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn fetch_rate_limits() -> Result<ProviderUsage, ProviderError> {
    let runtimes = DesktopRuntimeResolver::from_environment().resolve_candidates();
    if runtimes.is_empty() {
        return Err(ProviderError::Api(
            "未找到 ChatGPT/Codex Desktop Runtime 或 Codex CLI".into(),
        ));
    }
    let mut last_error = None;
    for runtime in &runtimes {
        match fetch_from_runtime(runtime) {
            Ok(usage) => return Ok(usage),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| ProviderError::Api("Codex Runtime 不可用".into())))
}

#[async_trait]
impl ProviderAdapter for CodexProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        _api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let mut usage = tokio::task::spawn_blocking(fetch_rate_limits)
            .await
            .map_err(|_| ProviderError::Api("Codex 额度查询任务失败".into()))??;
        usage.provider = config.provider_type.clone();
        Ok(usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dynamic_windows_without_fixed_durations() {
        let usage = parse_rate_limits(
            &json!({"rateLimits":{"limitId":"codex","limitName":"Codex","primary":{"usedPercent":18.0,"windowDurationMins":180,"resetsAt":2_000_000_000},"secondary":{"usedPercent":64.0,"windowDurationMins":10080,"resetsAt":2_000_100_000},"planType":"plus","credits":{"balance":12}}}),
            RuntimeSource::DesktopUserRuntime,
        ).unwrap();
        let codex = usage.codex.unwrap();
        assert_eq!(codex.windows.len(), 2);
        assert_eq!(codex.windows[0].remaining_percent, 82.0);
        assert_eq!(codex.windows[0].window_duration_mins, Some(180));
        assert_eq!(usage.remaining, Some(36.0));
    }

    #[test]
    fn missing_quota_is_not_reported_as_zero() {
        assert!(
            parse_rate_limits(&json!({"rateLimits":{}}), RuntimeSource::DesktopInstall).is_err()
        );
    }

    #[test]
    fn account_usage_populates_lifetime_tokens() {
        let mut usage = ProviderUsage::empty("codex");
        apply_account_usage(
            &mut usage,
            &json!({"summary":{"lifetimeTokens":1234},"dailyUsageBuckets":[]}),
        );
        assert_eq!(usage.total_tokens, 1234);
        assert_eq!(usage.today_tokens, None);
    }

    #[test]
    fn calculates_tokens_only_from_explicit_window_quota_fields() {
        let usage = parse_rate_limits(
            &json!({"rateLimits":{"primary":{"usedPercent":25,"tokenLimit":1000,"tokensUsed":250}}}),
            RuntimeSource::DesktopUserRuntime,
        ).unwrap();
        let window = &usage.codex.unwrap().windows[0];
        assert_eq!(window.token_limit, Some(1000));
        assert_eq!(window.tokens_used, Some(250));
        assert_eq!(window.tokens_remaining, Some(750));
    }

    #[test]
    fn does_not_infer_token_quota_from_percent_or_usage() {
        let usage = parse_rate_limits(
            &json!({"rateLimits":{"primary":{"usedPercent":25}}}),
            RuntimeSource::DesktopUserRuntime,
        )
        .unwrap();
        let window = &usage.codex.unwrap().windows[0];
        assert_eq!(window.token_limit, None);
        assert_eq!(window.tokens_used, None);
        assert_eq!(window.tokens_remaining, None);
    }

    #[test]
    fn source_does_not_read_auth_material() {
        let source = include_str!("codex.rs").to_ascii_lowercase();
        let production = source.split("#[cfg(test)]").next().unwrap_or(&source);
        for forbidden in [
            "read_to_string(",
            "std::fs::read(",
            ".bearer_auth(",
            "authorization\"",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden auth access: {forbidden}"
            );
        }
        assert!(production.contains("account/ratelimits/read"));
    }
}
