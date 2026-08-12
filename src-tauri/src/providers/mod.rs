//! Provider 抽象层：统一数据结构 + 平台适配 trait + Provider Manager
//!
//! 新增平台时只需实现 [`ProviderAdapter`] 并在 [`ProviderManager::new`] 注册，
//! 前端与调度层无需改动（mission.md §5 Provider 设计）。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod claude;
pub mod codex;
pub mod deepseek;
/// 暂未注册（官方无公开余额/用量查询端点）；保留实现与说明供未来启用。
#[allow(dead_code)]
pub mod gemini;
pub mod openai;
pub mod openrouter;
pub mod siliconflow;

/// 统一返回给前端的 Provider 用量数据（对应 mission.md 的 ProviderUsage 接口）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    /// 由命令层填充的 provider 记录 id，供前端精确关联卡片。
    pub provider_id: Option<i64>,
    pub provider: String,
    pub balance: Option<f64>,
    pub currency: String,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    /// 当日 Token（V0.5 口径）：Some(含 0) = 平台明确提供今日值；None = 未知/不提供。
    #[serde(default)]
    pub today_tokens: Option<u64>,
    pub today_cost: Option<f64>,
    pub month_cost: Option<f64>,
    pub remaining: Option<f64>,
    pub reset_time: Option<String>,
    pub updated_at: String,
}

impl ProviderUsage {
    /// 构造一个空壳数据（刷新失败或首次展示时使用）。
    pub fn empty(provider: impl Into<String>) -> Self {
        Self {
            provider_id: None,
            provider: provider.into(),
            balance: None,
            currency: String::new(),
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: 0,
            today_tokens: None,
            today_cost: None,
            month_cost: None,
            remaining: None,
            reset_time: None,
            updated_at: String::new(),
        }
    }
}

/// Provider 配置（数据库 providers 表的一行，序列化给前端）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub api_url: String,
    /// keyring 引用（service:key_<uuid>）。仅后端内部使用，不序列化给前端。
    #[serde(skip_serializing)]
    pub key_ref: String,
    pub enabled: bool,
    pub created_time: String,
    pub updated_time: String,
}

/// 凭据来源（P1：显式区分，避免用伪 key_ref 表示无凭据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialSource {
    /// 密钥存于系统凭据库（keyring），key_ref 为其引用。
    Keyring,
    /// 复用 Codex CLI 本地登录态（~/.codex/auth.json），key_ref 为空。
    CodexCli,
}

impl ProviderConfig {
    /// 凭据来源（按类型推导，Codex 为唯一 CLI 凭证类型）。
    pub fn credential_source(&self) -> CredentialSource {
        if self.provider_type == "codex" {
            CredentialSource::CodexCli
        } else {
            CredentialSource::Keyring
        }
    }
}

/// 构造分页下一页 URL（P1：cursor 视为 opaque token，必须 URL 编码，不能直接拼接）。
pub(crate) fn next_page_url(
    base_url: &str,
    param: &str,
    cursor: &str,
) -> Result<String, ProviderError> {
    let mut url = url::Url::parse(base_url)
        .map_err(|e| ProviderError::Api(format!("分页 URL 构造失败: {e}")))?;
    url.query_pairs_mut().append_pair(param, cursor);
    Ok(url.to_string())
}

/// 分页状态推进（纯函数，便于 mock 测试）。
/// 返回 Ok(true)=继续下一页；Ok(false)=正常结束（has_more=false 或 cursor 缺失）。
/// Err=重复 cursor（服务端异常）。
pub(crate) fn advance_page(
    has_more: bool,
    next_page: Option<&str>,
    seen: &mut std::collections::HashSet<String>,
    page: &mut Option<String>,
) -> Result<bool, ProviderError> {
    match next_page {
        Some(next) if has_more && !next.is_empty() => {
            if !seen.insert(next.to_string()) {
                return Err(ProviderError::Api(format!(
                    "分页游标重复（服务端异常）: {next}"
                )));
            }
            *page = Some(next.to_string());
            Ok(true)
        }
        _ => {
            // has_more=false 或 next_page 缺失：正常结束（P0：显式标记完成）
            *page = None;
            Ok(false)
        }
    }
}

/// Provider 查询过程中的错误，统一映射为前端可读信息。
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),
    #[error("API 返回错误: {0}")]
    Api(String),
}

/// 平台适配器 trait：每个 AI 平台实现一个，保持独立（mission.md 要求）。
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// 查询一次用量数据。
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError>;
}

/// Provider 管理器：类型 -> 适配器 注册表。
pub struct ProviderManager {
    registry: HashMap<String, Box<dyn ProviderAdapter>>,
}

impl ProviderManager {
    pub fn new() -> Self {
        let mut registry: HashMap<String, Box<dyn ProviderAdapter>> = HashMap::new();
        registry.insert("deepseek".into(), Box::new(deepseek::DeepSeekProvider));
        registry.insert("openai".into(), Box::new(openai::OpenAIProvider));
        registry.insert("codex".into(), Box::new(codex::CodexProvider));
        registry.insert(
            "openrouter".into(),
            Box::new(openrouter::OpenRouterProvider),
        );
        registry.insert(
            "siliconflow".into(),
            Box::new(siliconflow::SiliconFlowProvider),
        );
        registry.insert("claude".into(), Box::new(claude::ClaudeProvider));
        // Gemini 暂不注册：官方无公开余额/用量查询端点，注册会造成必然失败的账户（V0.4 复审 P1）。
        Self { registry }
    }

    /// 按 provider_type 取适配器。
    pub fn get(&self, provider_type: &str) -> Option<&dyn ProviderAdapter> {
        self.registry.get(provider_type).map(|b| b.as_ref())
    }

    /// 已注册的 Provider 类型列表（供前端下拉选择）。
    /// V0.4 复审 P2：排序保证顺序稳定（HashMap 遍历无序）。
    pub fn supported_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self.registry.keys().cloned().collect();
        types.sort();
        types
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{advance_page, next_page_url, ProviderError};
    use std::collections::HashSet;

    #[test]
    fn pagination_completes_normally_after_more_pages() {
        // P0：翻页后 has_more=false 必须正常完成（不误判为超限）
        let mut seen = HashSet::new();
        let mut page = Some("cursor1".to_string());
        // 第 1 页：more + cursor → 继续
        assert!(advance_page(true, Some("cursor2"), &mut seen, &mut page).unwrap());
        assert_eq!(page.as_deref(), Some("cursor2"));
        // 第 2 页：false → 正常结束，page 清空
        assert!(!advance_page(false, Some("cursor3"), &mut seen, &mut page).unwrap());
        assert_eq!(page, None, "正常结束应清空 page");
    }

    #[test]
    fn pagination_rejects_duplicate_cursor() {
        let mut seen = HashSet::new();
        let mut page = None;
        assert!(advance_page(true, Some("c1"), &mut seen, &mut page).unwrap());
        let err = advance_page(true, Some("c1"), &mut seen, &mut page);
        assert!(matches!(err, Err(ProviderError::Api(_))), "重复 cursor 应报错");
    }

    #[test]
    fn pagination_missing_cursor_is_complete() {
        // has_more=true 但 cursor 缺失：按正常结束处理（避免死循环）
        let mut seen = HashSet::new();
        let mut page = Some("x".to_string());
        assert!(!advance_page(true, None, &mut seen, &mut page).unwrap());
        assert_eq!(page, None);
    }

    #[test]
    fn next_page_url_encodes_cursor() {
        // P1：cursor 视为 opaque token，必须 URL 编码
        let url = next_page_url(
            "https://api.example.com/v1/x?start=1",
            "page",
            "a b&c=1",
        )
        .unwrap();
        assert!(url.contains("page=a+b%26c%3D1"), "cursor 应被 URL 编码: {url}");
        assert!(url.contains("start=1"), "原查询参数应保留: {url}");
    }
}
