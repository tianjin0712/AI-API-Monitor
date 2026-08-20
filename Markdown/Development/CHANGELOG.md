# Changelog

## 2026-08-19

- 新增通用自定义 API Provider（`custom`）：可配置请求方法（GET/POST）、URL、Query、Headers、认证方式（Bearer / API Key Header / Basic Auth / 无认证 / 自定义 Header）、JSON Body、响应字段点路径映射（remaining/total/used/resetTime）与单位（Token/次数/金额/自定义）。
- 新增测试连接能力（`test_custom_provider`），返回脱敏后的解析结果与响应结构预览，不写入用量历史、不改动 Provider 配置。
- 敏感值（Token/Key/Basic 密码/自定义 Header 值）仅经系统 keyring 存储；非敏感配置存 SQLite `providers.custom_config`（schema V7）。
- 扩展 JSON 递归脱敏与回环 HTTP 测试客户端；ProviderCard/MiniBall 按自定义单位展示剩余额度。

## 2026-08-13

- 完成项目迁移准备并同步到 `F:\AI API Monitor`，保留 Git 信息和源代码、资源、文档、脚本、MCP/开发配置及构建配置。
- 完成 macOS 兼容性检查：路径分隔符、文件大小写、环境变量、Shell/批处理脚本、构建工具、第三方依赖和平台条件代码。
- 准备 Miuix 风格 UI 重构，整理液态玻璃主题、输入框、选择框和基础 UI 控件。
- 整理壁纸系统：缩略图滚动、主题色识别、自定义主题颜色和用户主题保留策略。
- 保留 Provider、API Key、Token/额度监控以及 Codex/OpenAI 登录支持。
- 强化安全文档：系统凭据库、Key 脱敏、图片/GIF 资源、Cookie/token 保护。
- 新增 `README_Project_Status_2026-08-14.md`（已归档）和 `DEVELOPMENT_GUIDE.md`，记录迁移背景、macOS 开发流程和后续方向。
