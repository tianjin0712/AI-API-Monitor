//! SiliconFlow（硅基流动）Provider 适配器
//!
//! 端点：`GET {base}/user/info`（Bearer 认证），返回账户余额。
//! 响应（社区经验，官方文档未收录该页；实现做容错解析）：
//! ```json
//! { "code": 0, "data": { "balance": "19.00", ... }, "message": "..." }
//! ```
//! 该端点仅提供余额，无 Token 用量/费用/重置时间。

use super::{ProviderAdapter, ProviderConfig, ProviderError, ProviderUsage};
use async_trait::async_trait;
use serde::Deserialize;

pub struct SiliconFlowProvider;

#[derive(Debug, Deserialize)]
struct UserInfoData {
    /// 余额（字符串，如 "19.00"）；缺失视为未知
    #[serde(default)]
    balance: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    data: Option<UserInfoData>,
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

#[async_trait]
impl ProviderAdapter for SiliconFlowProvider {
    async fn fetch_usage(
        &self,
        config: &ProviderConfig,
        api_key: &str,
    ) -> Result<ProviderUsage, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let url = format!("{}/user/info", config.api_url.trim_end_matches('/'));
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

        let data: UserInfoResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Api(format!("响应解析失败: {e}")))?;

        if data.code != 0 {
            return Err(ProviderError::Api(
                data.message
                    .unwrap_or_else(|| format!("业务错误 code={}", data.code)),
            ));
        }

        let mut usage = ProviderUsage::empty(config.provider_type.clone());
        usage.currency = "¥".into(); // SiliconFlow 以人民币计费
        // balance 为字符串，缺失视为未知（容错）
        if let Some(balance) = data.data.and_then(|d| d.balance) {
            if !balance.trim().is_empty() {
                usage.balance = balance.trim().parse::<f64>().ok();
            }
        }
        usage.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_info_response() {
        let json = r#"{ "code": 0, "data": { "balance": "19.00", "status": "active" }, "message": "" }"#;
        let resp: UserInfoResponse = serde_json::from_str(json).expect("parse ok");
        assert_eq!(resp.code, 0);
        assert_eq!(resp.data.as_ref().and_then(|d| d.balance.as_ref()), Some(&"19.00".to_string()));
    }

    #[test]
    fn tolerates_missing_balance() {
        let json = r#"{ "code": 0, "data": {} }"#;
        let resp: UserInfoResponse = serde_json::from_str(json).expect("parse ok");
        assert!(resp.data.unwrap().balance.is_none());
    }

    #[test]
    fn parses_error_response() {
        let json = r#"{"code":30014,"data":null,"message":"Token is invalid."}"#;
        let resp: UserInfoResponse = serde_json::from_str(json).expect("parse ok");
        assert_ne!(resp.code, 0);
    }
}
