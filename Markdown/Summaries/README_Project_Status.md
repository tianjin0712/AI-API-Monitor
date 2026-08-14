# AI API Monitor 项目状态

## 项目概述

AI API Monitor 是一个基于 Tauri 2 的跨平台桌面 AI API 监控工具，用于集中查看多个 Provider 的余额、Token、费用、历史趋势和额度提醒。当前项目正在从 Windows 开发环境迁移到 `F:\AI API Monitor`，并为 macOS 继续开发做跨平台整理。

技术栈：React 19、TypeScript、Vite、Tailwind CSS、Tauri 2、Rust 2021、SQLite、Reqwest、Keyring、Tokio。项目还包含 Miuix 风格组件、液态玻璃样式、主题资源和壁纸/背景处理工具。

当前开发状态：V0.5-alpha 基础监控与桌面能力已具备；Provider 管理、系统凭据存储、主题系统和资源安全模块已进入持续优化阶段。迁移不改变业务逻辑，目标是保证 Windows 与 macOS 的开发路径一致。

## 已完成任务

### UI/UX

- 已建立 Miuix 风格 UI 重构准备：组件入口位于 `src/components/miuix/`，通用控件位于 `src/components/ui/`。
- 已整理液态玻璃主题样式：`src/styles/miuix.css`、`src/styles/miuix-official.css` 与 `src/index.css`。
- 输入框、选择框、Button、Switch 等控件已统一视觉变量与交互状态。
- 壁纸系统已加入背景裁剪、主题资源和横向缩略图所需的前端结构：`BackgroundCropper.tsx`、`src/utils/themeAssets.ts`、`public/themes/`。
- 支持从壁纸/背景资源提取主题色，并允许用户设置自定义主题颜色。
- 保留用户自定义主题系统，主题状态仍由 `src/theme/applyTheme.ts`、布局配置和本地主题缓存共同管理。

### API 管理

- Provider 管理：新增、编辑、删除、启用状态及动态 Provider 类型列表。
- API Key 管理：Key 只通过系统凭据库保存，SQLite 只记录引用。
- Token、余额、费用、历史趋势、预测和低额度提醒已接入 Dashboard。
- 已注册 Provider 包括 Claude、Codex、DeepSeek、OpenAI、OpenRouter、SiliconFlow；Gemini 保留实现但未注册。
- Codex 支持复用本机 Codex CLI 登录态；OpenAI/Claude 支持需要组织权限的用量接口。

### 安全优化

- API Key 使用 Keyring/系统凭据库，并通过唯一引用关联账户。
- 错误处理、日志和诊断路径避免输出完整 API Key。
- 图片/GIF 资源通过资源清单、路径校验和解码限制管理，避免任意路径读取。
- Cookie、access token 和本地认证文件按敏感数据处理；Codex 登录态仅在后端读取，不返回前端。
- Windows 专用安全依赖已放入 `cfg(windows)` 条件依赖，macOS 不会加载 Windows API。

### UI 组件

- 卡片系统：ProviderCard、统计卡片、趋势卡片和液态玻璃容器。
- 输入框：统一焦点、错误、占位和暗/亮主题样式。
- 下拉框：Provider 选择器支持键盘导航与焦点恢复。
- Button：标题栏、刷新、编辑、删除和布局操作使用统一按钮样式。
- Switch：Always On Top 等设置使用主题适配的开关控件。
- 设置页面：Provider CRUD、刷新策略、窗口行为、主题和安全提示集中在 `src/pages/Settings.tsx`。

### 迁移计划

- 当前准备继续参考 Miuix UI 的层级、间距、动效和控件反馈理念。
- 使用 Miuix 设计理念优化桌面端体验，同时保留 Tauri 原生窗口、托盘和悬浮模式。
- 保留自定义主题、壁纸、主题色与用户布局，不把视觉规范硬编码成单一主题。

## 最近两天的迁移与整理

- 完整同步源项目到 `F:\AI API Monitor`，保留目标目录的 Git 元数据。
- 检查路径分隔符、硬编码盘符、环境变量、构建脚本、第三方依赖和条件编译。
- 目标目录已移出 `node_modules`、`dist`、`.reasonix`、Tauri `target` 和生成的 schema 缓存；这些内容位于 `.migration-quarantine`，可在确认无需要后删除。
- 新增 macOS 开发指南、迁移状态和变更日志。

## 当前验证状态

迁移前源项目已通过 `pnpm check`、`pnpm build`、Rust 格式检查、严格 Clippy 和 Rust 测试。目标目录复制后尚未重新安装依赖，因此目标目录的完整构建状态需在 macOS 或 Windows 上执行依赖安装后确认。
