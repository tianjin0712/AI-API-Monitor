# Changelog

## 2026-08-23

- 新增 macOS 发布打包脚本 `scripts/Build-Release.sh`：以 Git Tag 为版本来源（精确 Tag > 最近可达 Tag > package.json 回退），注入 manifest 后构建 `.app` 与 `.dmg`，产物文件名末尾追加版本号，与 Windows 端等价。
- Windows 发布打包脚本 `scripts/Build-Release.ps1` 纳入版本管理并完善：根据 Git Tag 派生版本、注入 manifest、构建失败自动重试，并统一安装版与便携版产物命名。
- manifest 版本升至 `1.0.7`（尚未创建对应 Git Tag）。

## 2026-08-22

- Windows 打包以 Git Tag 作为版本来源：新增 `tools/package-windows.mjs`，按「精确 Tag > 最近可达 Tag > package.json 回退」派生版本并注入 manifest，保证产物文件名与应用内部版本同源一致。
- 加固数据库损坏恢复：恢复过程 sidecar 文件移动失败时支持原子回滚，避免从部分保留的 WAL 重建；legacy 凭据迁移改用专用解析方法，防止接受可预测的旧账号名。

## 2026-08-20

- 强化发布流程：新增统一版本字段模块，`pnpm version:sync` 必须显式传入目标版本；新增 `pnpm version:check` 与 `pnpm release:verify`，构建前改为只读版本检查，避免 Tag 构建改写源码。
- 新增 GitHub Release 工作流：由 `v*` Tag 触发，构建前再次校验 Tag 与三份 manifest 版本一致。
- CI 质量门禁限定为主分支提交与 Pull Request 触发；Tag 推送改由 Release 工作流负责，避免历史 Tag 重新推送时产生无意义失败。

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
