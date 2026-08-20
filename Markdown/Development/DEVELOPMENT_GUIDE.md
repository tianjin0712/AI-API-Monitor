# AI API Monitor 开发指南

## 启动项目

在项目根目录执行：

```bash
pnpm install --frozen-lockfile
pnpm tauri dev
```

仅调试前端时可执行 `pnpm dev`。Tauri 开发服务器固定使用 1420 端口；`TAURI_DEV_HOST` 只在需要局域网/远程 WebView 调试时设置，不要写死机器 IP。

## 安装依赖与编译

要求：Node.js 20.19+、22.12+ 或 24+，pnpm 11.21.0，Rust 1.97.1，Tauri 2 所需系统依赖。macOS 还需要 Xcode Command Line Tools；Windows 需要 WebView2 和 C++ 构建工具。

```bash
pnpm install --frozen-lockfile
pnpm check
pnpm build
pnpm tauri build
```

`pnpm check` 会执行 TypeScript、Vitest、Rust fmt/check/Clippy 和 Rust 测试。macOS 构建会使用 `src-tauri/tauri.conf.json` 中的通用配置和 macOS 图标；Windows 专用依赖由 Cargo 条件编译控制。

## 项目目录结构

```text
src/                         React/TypeScript 前端
├── components/              标题栏、卡片、Miuix 控件和背景裁剪
├── pages/                   Dashboard 与 Settings
├── styles/                  Miuix/液态玻璃样式
├── theme/                   主题应用逻辑
└── utils/                   格式化、布局、主题资源和测试
src-tauri/src/               Rust 后端
├── providers/               Provider trait 与平台适配器
├── db/                      SQLite 连接和迁移
├── commands.rs              Tauri 命令、刷新、历史、预测和提醒
├── security.rs              资源/敏感数据安全逻辑
├── assets.rs                图片/GIF 资源校验与读取
├── storage.rs               Keyring 凭据存储
└── platform_security.rs     平台条件安全能力
public/fonts/                字体资源
public/themes/               壁纸与 GIF 主题资源
Markdown/                    项目、测试、安全和审查文档
```

## UI 修改位置

- Dashboard：`src/pages/Dashboard.tsx`
- 设置页：`src/pages/Settings.tsx`
- Provider 卡片：`src/components/ProviderCard.tsx`
- 趋势图：`src/components/TrendWidget.tsx`
- 标题栏/窗口模式：`src/components/TitleBar.tsx`、`MiniBall.tsx`
- Miuix 组件：`src/components/miuix/`、`src/components/ui/`
- 液态玻璃和控件样式：`src/index.css`、`src/styles/`

优先复用现有 CSS 变量和组件，不在页面中新增硬编码颜色。修改主题时同时验证暗色、亮色、透明窗口和壁纸背景。

## 添加 API Provider

1. 在 `src-tauri/src/providers/` 新建适配器，实现 `ProviderAdapter`。
2. 在 `ProviderManager::new()` 注册类型。
3. 添加响应解析、错误响应、超时、非法数值和 URL 契约测试。
4. 如需特殊 URL、Key 或权限提示，同步更新 `src/pages/Settings.tsx` 与 `src/types.ts`。
5. 使用 mock 服务验证 401/403、429、5xx、超时和字段缺失。

API Key 只能进入系统凭据库，禁止写入 SQLite、普通日志、前端 DTO 或文档示例。

## 通用自定义 API Provider

`custom` 类型现为通用自定义 API 适配器（`src-tauri/src/providers/custom.rs`），不再复用 OpenAI Admin 协议。配置结构（非敏感部分，存 `providers.custom_config` JSON）：

```text
CustomApiConfig
- url            请求完整 URL（https；本机 http 仅测试连接放行回环地址）
- method         GET / POST
- query          [{key, value}]，结构化 URL 编码
- headers        [{key, value}]，认证头请用 auth 配置
- body           JSON 字符串（仅 POST 发送）
- auth           { type, headerName?, username? }
- responseMapping { remainingPath?, totalPath?, usedPath?, resetTimePath? }
- unit           token | count | currency | custom
```

认证方式（`auth.type`）：`bearer`（Authorization: Bearer）、`apiKey`（自定义/默认 `X-API-Key` 头）、`basic`（username + 密码，Base64）、`none`（无认证）、`customHeader`（自定义头名 + 值）。敏感值只经 `SecureStorage` 存入 keyring，SQLite 仅保存不可逆的 `key_ref`。

JSON 响应字段用点路径读取（支持嵌套对象与数组索引，如 `data.items.0.value`），支持数字与数字字符串，拒绝负数/NaN/Infinity。余额计算：`remaining` 优先；否则 `total - used`；只有 total 或 used 时保留可用字段，不以 0 伪装未知。

新增自定义 API 步骤：Settings → 类型选 `custom` → 填写名称、URL、方法、Query/Headers、认证、Body、字段映射、单位 → 点「测试连接」验证 → 保存。测试连接返回脱敏后的解析结果与响应结构预览，不写入历史。

## 主题系统位置

- 主题状态与应用：`src/theme/applyTheme.ts`
- 布局与主题解析：`src/utils/layout.ts`
- 主题资源：`src/utils/themeAssets.ts`
- Miuix 样式：`src/styles/miuix.css`、`src/styles/miuix-official.css`
- 壁纸资源：`public/themes/`
- 自定义背景裁剪：`src/components/BackgroundCropper.tsx`

新增主题资源时使用相对 URL 和小写文件名，避免 macOS 大小写敏感文件系统下出现引用不一致。

## 配置文件

- 前端依赖和脚本：`package.json`、`pnpm-lock.yaml`
- Vite/Tauri 开发服务器：`vite.config.ts`
- TypeScript：`tsconfig.json`、`tsconfig.node.json`
- Rust 依赖：`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- Rust 工具链：`rust-toolchain.toml`
- Tauri 窗口、CSP 和打包：`src-tauri/tauri.conf.json`
- Tauri 权限：`src-tauri/capabilities/default.json`
- CI：`.github/workflows/quality.yml`

## macOS 开发注意事项

- 使用 `/` 作为路径分隔符；代码中不要拼接盘符或 Windows 反斜杠。
- 文件名和导入路径必须大小写完全一致。
- 不依赖 `.bat`、PowerShell 或 `cmd.exe`；使用 pnpm、Cargo 和跨平台 Node/Rust API。
- 不把 `HOME`、Keychain 路径或临时目录写死；使用 Tauri `app.path()`、Rust `dirs`/标准 API 或系统凭据库。
- `cfg(windows)` 代码只能放在 Windows 平台模块；macOS 构建应跳过 `windows-sys`。
- 修改资源、权限或打包配置后，在目标平台分别执行 `pnpm check` 和 `pnpm tauri build`。
