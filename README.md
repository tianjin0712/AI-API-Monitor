# AI API Monitor

AI API Monitor 是一个基于 Tauri 2 的跨平台桌面监控工具，用于集中查看多个 AI Provider 的余额、Token、费用、额度窗口与刷新状态。最近发布基线 Tag 为 `v1.0.7`（通用自定义 API Provider、发布流程与 Windows/macOS 打包脚本、数据库恢复加固），Tag 指向 `chore: release v1.0.7` 发布提交，manifest 版本一致。产品成熟度仍按 **V1.0-alpha** 管理：Windows 安装版已完成基础实际验收；数据库恢复 P0、macOS 真机验证、生产签名和自动更新端到端验收尚未完成。

## 项目简介

- **定位**：类似 GPU Monitor / Rainmeter / macOS Widget 的 AI 资源监控中心。
- **已支持 Provider**：DeepSeek、OpenAI、Codex、OpenRouter、Claude、SiliconFlow；Gemini 适配器保留但未注册（官方公开查询端点不足）。
- **技术栈**：Tauri 2、Rust 2021、React 19、TypeScript、Vite 7、Tailwind CSS 4、SQLite（WAL/迁移）、reqwest、tokio、keyring。
- **开发环境**：Node.js 20.19+（或 22.12+）、pnpm 11、Rust stable、对应平台的 Tauri WebView/构建工具。
- **当前状态**：Windows 安装包已在实际安装环境验证，安装后核心日常功能可正常使用；数据库恢复与迁移安全仍需完整人工复验。macOS 构建、Keychain、通知、窗口行为和签名发布尚未在本机完成验收。
- **发布口径**：发布前必须保证 Git Tag、`package.json`、`Cargo.toml` 和 `tauri.conf.json` 的版本一致；下一次发布按 `docs/RELEASE.md` 流程同步受控 manifest、创建 release commit 并打 annotated Tag。

## Windows 安装验收

已完成 Windows 安装版实际验收：安装、启动、Dashboard、Provider 管理与刷新、主题资源、托盘与悬浮窗、设置持久化、开机自启、置顶、拖拽、多显示器和不同 DPI 缩放均可正常使用。Codex Runtime 子进程已验证不再弹出控制台。普通用户使用安装包时无需安装 Node.js、pnpm、Rust、Cargo、Python 或 Git。

安装与发布流程见 [Windows 发布说明](./docs/RELEASE.md)。

## 当前完成内容

### UI 系统

- Dashboard、Settings、ProviderCard、TitleBar、MiniBall、TrendWidget 和统一控件已实现。
- Miuix/液态玻璃方向已落到 `src/styles/miuix.css`、`miuix-official.css` 和 `src/components/miuix/`：卡片、输入框、下拉框、Button、Switch、Tooltip 与焦点态使用统一 token。
- Widget 支持排序、显示/隐藏、添加/删除（每类唯一实例），布局 JSON 持久化。
- 待优化：完整 DIY（自由定位、缩放、透明度、字体/颜色）、真实桌面端触摸/拖动和多屏回归。

### API 管理

- Provider CRUD、启用/禁用、逐账户刷新和全量刷新已由 Rust commands 与 `ProviderManager` 统一调度。
- 当前适配器位于 `src-tauri/src/providers/`；HTTP 合约 Mock 与分页/错误分支测试已加入。
- API Key 只写入系统凭据库（Windows Credential Manager / macOS Keychain），SQLite 仅保存唯一引用和脱敏 hint；日志统一脱敏。
- 刷新间隔由后端校验：前台 10–3600 秒、后台 60–3600 秒，后台不能快于前台。

### Codex 支持

- 登录由官方 Codex/ChatGPT CLI 负责，应用只调用 `login` 和 `login status`，不读取 `auth.json`、Cookie、Token 或浏览器数据。
- 额度读取使用官方运行时的 App Server stdio 通道，监听 `account/rateLimits/updated`，解析 primary/secondary 窗口、使用率、剩余率、重置时间及可用 token 字段。
- 运行时解析支持桌面用户运行时、桌面安装、打包运行时和独立 CLI；后续需记录各版本 CLI 的兼容性，并等待稳定公开额度接口。

### 悬浮窗

代码内部名称为 `Full / Mini / Ball`，产品文档统一映射为：

| 产品状态 | 代码状态 | 尺寸 | 交互 |
|---|---|---:|---|
| `MAIN` | `Full` | 460×720 | 完整 Dashboard、设置、Widget 编辑 |
| `FLOAT_SQUARE` | `Ball` | 96×96 | 悬浮方块/小球，点击展开 |
| `FLOAT_EXPANDED` | `Mini` | 280×96 | 紧凑状态条，可切换回主界面 |

- 状态由 Rust `window_mode.rs` 管理，React 通过 `window-mode-changed` 同步；几何位置按模式保存。
- 托盘支持模式切换、显示/隐藏、退出；Tooltip 使用独立窗口并进行屏幕边界夹紧。
- 已解决：托盘切换与 React 状态不同步、Mini/Ball 位置持久化和 Tooltip 基础定位。
- Windows 已实测：托盘、Full/Mini/Ball 切换、关闭到托盘、开机自启、置顶、拖拽、多显示器及不同 DPI 缩放均正常；Codex Runtime 子进程不再弹出控制台。macOS 的窗口、DPI、多屏与交互仍需真机手测。

### 主题系统

- 亮/暗主题、主题 token 和自定义颜色覆盖持久化于布局设置。
- 支持用户背景/壁纸资源导入、裁剪、资源隔离、主题色提取与对比度调整；GIF/图片导入在后端进行格式、大小、帧数和 SVG 限制。
- 自定义主题系统保留，后续可继续沿 Miuix 设计语言扩展分享/导入主题。

### 设置系统

- 已实现 Provider 添加/编辑/删除、刷新间隔、主题、Widget 布局、置顶、关闭行为、自动启动和更新检查入口。
- 已实现但待真实环境验收：旧凭据 UUID 迁移、凭据删除失败补偿、数据库损坏安全启动、迁移前快照。
- 待完善：诊断导出、面向用户的数据库备份/恢复入口、自动更新生产公钥和端点。

## TODO

### 高优先级

- 完成数据库恢复、迁移快照、凭据迁移/补偿与敏感数据泄漏的发布前人工验收；恢复并追踪对应验收清单。
- 完成 macOS/Linux 真机的窗口、通知、Keychain/Secret Service、DPI 和多屏回归；Windows 在发布前仅需针对恢复与升级场景复回归。
- 为真实 Provider 账户补充权限、限流、分页、错误和 Codex CLI 版本兼容验证。
- 配置自动更新生产公钥、HTTPS endpoint、签名产物，并完成篡改/降级验收。

### 中优先级

- 完成完整 DIY UI 与桌面 E2E 测试，覆盖拖动、Tooltip、DPI 和窗口恢复。
- 为既有旧凭据迁移和删除失败补偿补齐故障注入与真实凭据库验收；实现诊断导出及面向用户的数据库备份/恢复。
- 统一文档中的 Full/Mini/Ball 与 MAIN/FLOAT_SQUARE/FLOAT_EXPANDED 命名，减少产品层歧义。

### 低优先级

- Provider 插件化加载、主题分享、更多平台适配（Gemini 等）。
- 更完整的日报/周报/月报、成本预测和跨设备配置同步。

## Changelog

### 2026-08-23

- 新增 macOS 发布打包脚本 `scripts/Build-Release.sh`，与 Windows 端一致以 Git Tag 派生版本并注入 manifest，产出 `.app` 与 `.dmg`。
- Windows 发布脚本 `scripts/Build-Release.ps1` 纳入版本管理并完善版本派生、构建重试与产物重命名。
- manifest 版本升至 `1.0.7`，并已创建对应 Git Tag `v1.0.7`。

### 2026-08-22

- Windows 打包以 Git Tag 为版本来源（`tools/package-windows.mjs`），保证产物文件名与应用内部版本同源。
- 加固数据库损坏恢复的 sidecar 回滚与 legacy 凭据迁移解析。

### 2026-08-20

- 强化发布流程：新增 `pnpm version:check` 与 `pnpm release:verify`，强制 Tag、发布提交与三份 manifest 版本一致；构建前只做只读版本检查。
- 新增 GitHub Release 工作流（`v*` Tag 触发）；CI 质量门禁限定为主分支提交与 Pull Request。

### 2026-08-19

- 数据库损坏恢复测试修正 Windows 下 recovery 主数据库与 WAL/SHM sidecar 的文件匹配。
- 增加版本同步工具和 `cargo check` 门禁；后续发布仍须在打 Tag 后验证受控 manifest 与安装包版本一致。
- Codex Windows 子进程统一使用无控制台创建方式；Windows 安装版已验证不再弹出空白控制台。

### 2026-08-17

- Windows 实测通过托盘、Full/Mini/Ball 切换、关闭到托盘、开机自启、置顶、拖拽、多显示器和不同 DPI 缩放。
- Codex Runtime 的 `login status` 与 `app-server` 子进程改为 Windows 无控制台创建；安装版已验证不再反复弹出空白控制台窗口。

### 2026-08-16

- Windows 安装版完成实际安装验收，现有功能安装后均可正常使用。
- 发布方式改为普通用户安装版，并补充一键发布脚本、便携版说明、用户数据隔离与启动失败日志说明。
- 悬浮窗逻辑与状态说明整理。
- Tooltip 行为和边界定位文档化。
- API 额度展示与 Codex App Server 读取方案补充。
- Miuix/液态玻璃 UI 重构方向归档。
- Markdown 文档按 Architecture、UI、API、Development、Review、TODO、Archive 重新分类。

## 文档索引

- [架构与历史方案](./Markdown/Architecture/mission.md)
- [UI 状态与设计方向](./Markdown/UI/UI_Status.md)
- [Provider 与安全矩阵](./Markdown/API/Provider_Matrix.md)
- [开发指南](./README_Project.md) · [跨平台指南](./Markdown/Development/DEVELOPMENT_GUIDE.md)
- [项目索引](./Markdown/Development/项目索引.md)
- [当前 TODO](./Markdown/TODO/TODO.md) · [历史审查](./Markdown/Review/codereview.md)
- [测试用例](./Markdown/Development/TEST_CASES.md) · [安全审计](./Markdown/API/Security_Audit_Report.md)

## 开发命令

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
pnpm typecheck
pnpm test
pnpm rust:fmt
pnpm rust:clippy
pnpm rust:test
```

Windows 可使用 `Start_AI_API_Monitor.bat`；macOS 请在 macOS 上安装 Rust、Node/pnpm 和 Xcode Command Line Tools 后执行同等 pnpm/Tauri 命令。不要把 Windows 绝对路径写入配置或脚本。
