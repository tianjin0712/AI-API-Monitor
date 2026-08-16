# AI API Monitor 安全审计报告

审计日期：2026-08-13  
范围：Tauri/Rust 后端、React/TypeScript 前端、SQLite、系统凭据库、Provider 网络请求、Codex 状态、图片/GIF、自定义主题、日志、自动更新、悬浮窗与权限配置。

## 执行摘要

项目原本已经使用 `keyring` 避免 API Key 明文落入 SQLite，但存在五类高优先级缺口：Codex 直接读取认证文件中的 access token、Provider 网络客户端缺少统一的重定向/代理策略、远端错误正文可能进入 UI、图片/GIF 以 Data URL 保存在 WebView 存储、日志缺少统一脱敏。以上问题均已修复。

当前安全基线为：API Key 存入平台凭据库；敏感设置使用 Keyring 主密钥保护的 AES-256-GCM；所有携带凭据的 API 请求只允许 HTTPS、验证证书、禁止重定向和隐式代理；自定义网关还要求显式授权，并阻断本机/私网目标和 DNS 重绑定；Codex 仅检查公开 CLI 登录状态；图片进入应用隔离目录并以不含用户路径的内部协议加载；Telemetry 默认关闭；自动更新未配置签名时保持禁用，且只允许语义版本升级。

## 关键位置

| 项目 | 位置 | 当前实现 |
|---|---|---|
| API Key 存储 | `src-tauri/src/storage.rs:11-90` | Windows Credential Manager、macOS Keychain、Linux Secret Service（由 `keyring` 后端提供） |
| 登录认证 | `src-tauri/src/providers/codex.rs:19-56` | 只运行 `codex login status`，丢弃输出 |
| OpenAI/Codex | `src-tauri/src/providers/openai.rs`、`providers/codex.rs` | OpenAI 从 Keyring 临时取 Key；Codex 不接触 Token/Cookie |
| 网络模块 | `src-tauri/src/security.rs:72-92` | 统一 HTTPS-only、TLS、无代理、无重定向、状态错误过滤 |
| 图片/GIF | `src-tauri/src/assets.rs:13-256` | 白名单、魔数、大小/尺寸/帧数、隔离目录、内部协议 |
| 配置/CSP | `src-tauri/tauri.conf.json:27-36` | 生产 CSP、更新器默认禁用 |
| 日志 | `src-tauri/src/security.rs:34-99` | `SensitiveDataFilter` 与统一安全日志入口 |
| 数据库/文件权限 | `src-tauri/src/db/mod.rs`、`platform_security.rs` | WAL、版本化迁移、`secure_settings`、Windows 受保护 DACL、Unix 0600/0700 |
| 自动更新 | `src-tauri/src/commands.rs` | 必须配置签名公钥和 HTTPS endpoint；禁止降级/重复安装/版本替换 |
| 悬浮窗 | `src/components/MiniBall.tsx` | 仅接收 Provider 名称、额度/用量状态与视觉资源 |

## 已修复发现

### AAM-001 — Critical — Codex 私有认证材料读取

- 位置：原 `src-tauri/src/providers/codex.rs`；修复后 `public_login_status`（19-32）。
- 证据：旧实现读取 `~/.codex/auth.json` 并把 `access_token` 作为 Bearer 发送。
- 影响：本地认证 Token 可能因日志、恶意 endpoint、重定向或进程内错误而泄露。
- 修复：删除所有认证文件解析和 Bearer 请求，只观察 `codex login status` 退出码；stdout/stderr 均重定向到 null。
- 缓解：SEC-003/004 静态回归门禁阻止重新引入文件读取与 Bearer 调用。
- 误报说明：无；旧代码存在明确的数据读取路径。

### AAM-002 — High — Provider 凭据可能发送到非预期地址

- 位置：`src-tauri/src/settings.rs:42-94`、`src-tauri/src/security.rs:72-81`。
- 证据：旧实现允许修改内置 Provider URL，且客户端默认跟随重定向/系统代理。
- 影响：Key 可能被发送到恶意 HTTPS 主机、代理或重定向目标。
- 修复：内置 Provider 固定官方 Registry 地址；自定义网关必须使用 `custom` 类型并首次显式授权；拒绝 IP、本机/内部域名以及解析到私网、链路本地、保留地址的域名；解析结果固定到请求客户端，降低 DNS 重绑定风险；客户端强制 HTTPS、禁用重定向和隐式代理。
- 缓解：UI 对内置 Provider URL 只读，并在自定义网关发送 Key 前展示目标 origin 与信任警告；远端错误正文不返回。
- 误报说明：用户仍可明确批准公网 HTTPS 网关，这是自定义 Provider 的必要产品能力。

### AAM-003 — High — 图片/GIF 未隔离且缺少深度校验

- 位置：`src-tauri/src/assets.rs:51-207`、`src/utils/customBackground.ts`、`src/utils/themeAssets.ts`。
- 证据：旧实现把 Data URL 写入 localStorage，仅按 MIME/4MB 检查 GIF。
- 影响：大图/高帧 GIF 可造成资源耗尽；恶意 SVG/伪扩展文件可成为活动内容；WebView 存储承载用户图片。
- 修复：导入字节复制至应用 `assets` 私有目录；真实格式、扩展名、SVG 活动内容、20MB、4096×4096、300 帧校验；随机资源 ID 与 `app-resource` 协议。
- 缓解：响应带 `nosniff` 与 `default-src 'none'; sandbox`，协议只接受安全文件名。
- 误报说明：内置主题文件属于打包资源，不经过用户导入路径。

### AAM-004 — High — 日志与错误缺少统一脱敏

- 位置：`src-tauri/src/security.rs:13-99`、`src-tauri/src/settings.rs:32-36`。
- 证据：旧 Provider 将完整远端错误正文回传，零散日志会输出 `key_ref`。
- 影响：响应回显或错误上下文可能把 Authorization、Token、Cookie、Password、Secret 写入 UI/日志。
- 修复：统一 `SensitiveDataFilter`；所有序列化到前端的 `AppError` 再脱敏；远端响应正文不再进入错误；日志统一走 `safe_log`。
- 缓解：SEC-002/011 覆盖常见格式。
- 误报说明：Provider 名称与凭据引用本身不再写日志。

### AAM-005 — Medium — 敏感设置缺少字段级加密

- 位置：`src-tauri/src/security.rs:101-139`、`src-tauri/src/settings.rs:319-398`、`src-tauri/src/db/mod.rs:121-136`。
- 证据：旧 `settings` 表仅提供明文 key/value。
- 影响：未来或遗留 Token/Cookie/Secret 若误写设置表会明文驻留。
- 修复：新增 `secure_settings`；32 字节主密钥只存平台 Keyring；字段使用随机 96-bit nonce 的 AES-256-GCM；启动时迁移敏感键名并删除明文。
- 缓解：解密结果使用 `Zeroizing`；Windows 目录/文件应用仅 SYSTEM 与对象所有者可访问的受保护 DACL，Unix 使用 0700/0600。
- 误报说明：额度、Provider 名称、刷新间隔等非敏感字段仍可明文保存。

### AAM-006 — Medium — 自动更新发布配置缺失

- 位置：`src-tauri/tauri.conf.json:34-37`、`src-tauri/src/commands.rs:508-590`。
- 证据：签名公钥为空且 endpoints 为空。
- 影响：如果在未配置签名的情况下启用更新，可能形成供应链风险。
- 修复：配置不完整时更新命令明确安全禁用；endpoint 只允许 HTTPS 且禁止 URL 内嵌凭据；安装由 Tauri Updater 验证签名；应用额外比较语义版本，只接受高于当前版本且与用户刚确认版本完全一致的更新。
- 缓解：SEC-014 阻止“有 endpoint、无公钥”配置。
- 误报说明：当前不是可利用漏洞，因为更新功能保持禁用。

### AAM-007 — Low — 不必要的 opener 权限

- 位置：`src-tauri/capabilities/default.json`、`src-tauri/src/lib.rs`。
- 证据：项目未使用 opener，但曾加载默认 opener 权限和插件。
- 影响：扩大前端可调用的系统能力面。
- 修复：移除 opener 插件、前端依赖和 capability。
- 缓解：继续按最小权限审查新增 Tauri 插件。
- 误报说明：代码搜索确认无调用点。

## 未解决/接受的风险

1. 自动更新尚无生产签名公钥与 HTTPS 发布地址，因此功能保持安全禁用；发布前必须由发布者提供真实签名配置并完成合法包、篡改包和降级包验证。
2. 用户明确批准的公网 `custom` Provider 会收到其 API Key。应用可以阻断技术型 SSRF 与误配置，但无法证明第三方运营方可信；企业部署可进一步配置组织域名 allowlist。
3. 本次环境只在 Windows 完成编译/测试；macOS Keychain 与 Linux Secret Service 需要各自真实系统会话的发布前集成测试。
4. 没有实现证书固定（pinning）。当前使用 Rustls 严格验证信任链；若威胁模型包含已受控根 CA，可为固定官方 Provider设计可轮换 pin 集。
5. 本机尚未安装 `cargo-audit`；项目和 CI 已加入 RustSec 审计命令，需在 CI 或安装该工具后执行并处置结果。

## 后续建议

1. 在 Windows、macOS、Linux 三平台 CI/真实桌面会话运行 Keyring、资源目录权限和升级迁移集成测试。
2. 配置 Tauri updater 的发布公钥、HTTPS endpoint 和签名产物验证，保留当前 fail-closed 守卫。
3. 企业版可在现有自定义网关显式授权上增加管理员域名 allowlist 与可轮换证书 pin 管理。
4. 保持 CI 的 `pnpm audit`、RustSec、SEC-001～015、严格 Clippy 和产物签名验证为发布阻断项。
