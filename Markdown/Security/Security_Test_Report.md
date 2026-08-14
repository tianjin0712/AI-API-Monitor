# AI API Monitor Security Test Report

测试日期：2026-08-13  
测试环境：Windows / Rust 1.97.1 / Node + pnpm / Tauri 2

## 测试矩阵

| ID | 验证目标 | 自动化证据 | 结果 |
|---|---|---|---|
| SEC-001 | API Key 不明文保存 | `security_tests::sec_001_api_key_uses_platform_keyring`；SQLite schema 无 Key 列 | PASS |
| SEC-002 | 日志不输出 Key | `sec_002_logs_redact_keys`、`security::tests::redacts_common_secret_forms` | PASS |
| SEC-003 | 不读取 Cookie/认证存储 | `sec_003_codex_does_not_read_cookie_or_auth_stores` | PASS |
| SEC-004 | Codex 不泄露 Token | `sec_004_codex_only_checks_public_cli_status`；命令输出丢弃 | PASS |
| SEC-005 | 图片不上传且路径隔离 | `sec_005_assets_use_opaque_app_resource_urls`；CSP 禁止前端外连 | PASS |
| SEC-006 | GIF 大小/尺寸/帧数 | `sec_006_gif_limits_are_enforced` + 解码器校验 | PASS |
| SEC-007 | 拒绝恶意扩展 | `sec_007_executable_extensions_are_not_allowlisted`、asset traversal/SVG tests | PASS |
| SEC-008 | HTTPS/TLS 验证 | `sec_008_http_client_enforces_tls`、HTTP URL 拒绝测试 | PASS |
| SEC-009 | 代理/重定向/SSRF 安全 | `sec_009_proxy_and_redirect_header_leaks_are_blocked`；私网解析与自定义网关授权测试 | PASS |
| SEC-010 | 敏感数据 AES-256-GCM | `sec_010_sensitive_fields_use_aes_256_gcm` + 篡改检测 | PASS |
| SEC-011 | 崩溃/UI 错误脱敏 | `sec_011_crash_and_ui_errors_are_redacted`；`AppError::serialize` 过滤 | PASS |
| SEC-012 | 配置/数据文件权限 | `sec_012_local_files_have_private_permission_controls`；Windows 真实 DACL 测试；Unix 0600/0700 | PASS |
| SEC-013 | 悬浮窗无敏感字段 | `sec_013_floating_window_has_no_secret_fields` | PASS |
| SEC-014 | 更新包验证 | `sec_014_updates_are_disabled_without_signature_config`；语义版本升级/降级测试 | PASS* |
| SEC-015 | 异常退出恢复 | `sec_015_database_uses_wal_and_versioned_migrations`；SQLite WAL/幂等迁移 | PASS |

`PASS*` 说明：SEC-014 当前以“未配置则禁用”通过，代码已阻止降级和版本替换；生产签名/更新源仍需发布者配置后再做端到端验证。

## 执行结果

- React/TypeScript 生产构建：PASS。
- Rust 单元与 SEC 测试：PASS（80 项，0 失败）。
- Rust 格式、Clippy（`-D warnings`）、全量测试：PASS。
- 日志静态检查：未发现 Provider 响应正文、完整 Key、Bearer、Cookie 或 Token 的直接输出路径。
- 前端生产构建：PASS；产物扫描仅命中 UI 中的示例前缀 `sk-ant-admin01-`，未发现真实 Key/Token。
- 前端依赖审计：官方 npm registry 最终报告 `No known vulnerabilities found`（首次连接发生超时重试）。
- Rust 依赖审计：已加入 `pnpm rust:audit`、`pnpm security:audit` 与 CI RustSec 阻断任务；本机未安装 `cargo-audit`，需由 CI 或安装后执行。

## 手工发布前检查

1. 在三平台分别新增、编辑、删除 Provider，确认系统凭据库创建/更新/删除且 SQLite 无完整 Key。
2. 导入 20MB 边界 GIF、4096 边界图、301 帧 GIF、伪装扩展和活动 SVG。
3. 使用无效证书、HTTP endpoint、30x 跨域重定向和系统代理环境验证请求 fail-closed。
4. 检查应用数据目录、崩溃日志与系统日志中没有测试 Key/Token/Cookie。
5. 配置真实 updater 公钥和 HTTPS endpoint 后验证合法包、篡改包、错误签名与降级版本。
