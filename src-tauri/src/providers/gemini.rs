//! Google Gemini Provider 适配器
//!
//! ⚠️ Gemini API 无公开的余额/用量查询端点（官方 Billing 文档明确：
//! 余额管理与交易历史只能在 Google AI Studio Billing 页面操作）。
//! 适配器保留注册以便 UI 展示该平台，但刷新时返回可行动说明。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;

pub struct GeminiProvider;

/// 无公开查询端点时的提示信息（同时展示在前端错误条）。
pub const GEMINI_NOT_SUPPORTED: &str =
    "Gemini API 无公开的余额/用量查询端点，请在 Google AI Studio 的 Billing 页面查看（aistudio.google.com）";

#[async_trait]
impl ProviderAdapter for GeminiProvider {
    async fn fetch_usage(
        &self,
        _config: &ProviderConfig,
        _api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        Err(ProviderError::Api(GEMINI_NOT_SUPPORTED.into()))
    }
}
