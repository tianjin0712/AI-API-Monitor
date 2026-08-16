# AI API Monitor Windows 发布说明

## 面向用户的下载内容

- **安装版（推荐）**：`AI API Monitor_<version>_x64-setup.exe`。双击后按向导安装；采用当前用户安装，不需要管理员权限，也不会显示开发终端。
- **便携版**：`AI-API-Monitor-portable.zip`。解压后双击 `AI API Monitor.exe`；首次运行仍需要 Windows WebView2 Runtime（Windows 11 通常已内置）。便携版的数据仍按 Windows 用户保存在应用数据目录，不会写入解压目录。

两种版本都已经内置应用 EXE、前端页面、JavaScript 依赖、Rust 依赖、SQLite 引擎、字体、主题图片和 GIF。用户不需要安装 Node.js、pnpm、Rust、Cargo、Python、Git 或项目源码。

## Windows 安装验收结果

Windows 安装版已完成实际安装测试。安装后，应用启动、Dashboard、Provider 管理与刷新、主题图片/GIF、托盘、Full/Mini/Ball 窗口模式、设置持久化、开机自启以及用户数据隔离均验证可正常使用。

## 用户数据与故障排查

- SQLite 数据库、主题设置、Provider 配置、用户导入图片与日志均存放在 Tauri 的**当前用户应用数据目录**，与安装目录分离；升级或重装不会覆盖它们。
- API Key 不写入数据库和日志，继续由 Windows Credential Manager 保护；数据库只保存引用和脱敏提示。
- 启动失败会弹出中文提示，不会打开终端窗口，并把已脱敏日志写入应用数据目录的 `logs/application.log`。常见原因是企业策略阻止应用数据目录写入；请联系管理员授予当前用户写入权限后重试。
- 设置页的“开机启动”使用 Windows 当前用户启动项，无需管理员权限。

## 自动更新预留

应用已包含 Tauri updater 接口，但在未配置签名公钥和 HTTPS 更新源前会安全禁用。正式启用时必须：

1. 生成并妥善保存 updater 私钥；将公开密钥填入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。
2. 配置仅 HTTPS 的 `endpoints`，发布由私钥签名的更新元数据和安装包。
3. 在干净 Windows 用户账户完成安装、升级、回滚失败和数据保留验证后再发布。

## 发布者构建流程

在已配置 Node/pnpm、Rust 与 Windows 构建依赖的发布机执行：

```powershell
pnpm install --frozen-lockfile
.\scripts\Build-Release.ps1
```

脚本先运行检查，再生成免管理员 NSIS 安装包和 ZIP 便携版到 `release/`。仅在已完成独立质量检查时才可使用 `-SkipChecks`。

发布目录：

```text
release/
├─ AI API Monitor_<version>_x64-setup.exe
├─ AI-API-Monitor-portable.zip
└─ AI-API-Monitor-portable/
   ├─ AI API Monitor.exe
   └─ RELEASE.md
```
