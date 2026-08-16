# Provider 与安全矩阵

| Provider | 代码适配器 | 当前状态 | 凭据/数据来源 |
|---|---|---|---|
| DeepSeek | `src-tauri/src/providers/deepseek.rs` | 已注册 | 系统凭据库中的 API Key |
| OpenAI | `openai.rs` | 已注册 | 系统凭据库中的 API Key |
| Codex | `codex.rs` | 已注册，实验性 | 官方 CLI/App Server；不读取认证文件 |
| OpenRouter | `openrouter.rs` | 已注册 | API Key；余额、费用和重置信息 |
| Claude | `claude.rs` | 已注册 | Anthropic Admin Key；Usage/Cost |
| SiliconFlow | `siliconflow.rs` | 已注册，端点实验性 | API Key；余额 |
| Gemini | `gemini.rs` | 保留实现，未注册 | 官方公开查询端点不足 |

## 统一数据路径

`commands.rs` 调用 `ProviderManager`，适配器返回 `ProviderUsage`，刷新结果写入 SQLite `usage_history`，前端通过 `src/api.ts` 和状态层展示。HTTP 合约测试位于 `src-tauri/src/providers/http_contract_tests.rs`。

## 安全边界

- API Key 由 `keyring` 保存，数据库仅保存唯一引用和脱敏 hint。
- `security.rs` 统一脱敏网络错误和日志；图片/GIF 资源由 `assets.rs` 做格式、大小、帧数和 SVG 限制。
- Codex 的登录材料由官方运行时持有，应用不打开 `auth.json`、Cookie、Token 或浏览器数据库。
- 生产发布前仍需完成真实权限/限流、密钥删除失败补偿和 macOS Keychain 验收。
