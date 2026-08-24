# AI API Monitor 项目维护说明

## 项目介绍

AI API Monitor 是一个 Tauri 2 桌面应用，用统一 Dashboard 展示多个 AI 平台的余额、Token、费用、历史趋势和预计耗尽时间。最近发布基线 Tag 为 `v1.0.6`（manifest 版本已升至 `1.0.7`，尚未打 Tag），产品成熟度按 V1.0-alpha 管理。

当前已注册平台：Claude、Codex、DeepSeek、OpenAI、OpenRouter、SiliconFlow，以及通用自定义 API（`custom`）。Gemini 代码仅保留说明，未对用户开放。

## 环境配置

必需工具：

- Node.js：`^20.19.0 || ^22.12.0 || >=24.0.0`；CI 使用 Node.js 22。
- pnpm：`11.21.0`（由 `packageManager` 固定）。
- Rust/Cargo：`1.97.1`、Rust 2021 edition（由 `rust-toolchain.toml` 固定）。
- Tauri 2 系统依赖：Windows 需要 WebView2 和 C++ 构建工具；其他系统按 Tauri 2 平台要求安装。

安装 JavaScript 依赖：

```bash
pnpm install
```

Rust 依赖由 Cargo 根据 `src-tauri/Cargo.lock` 获取。仓库没有提交 `.env` 模板，Provider 凭据由应用 UI 写入系统凭据库，不应写入源码或配置文件。

## 启动与构建

开发模式：

```bash
pnpm tauri dev
```

仅启动前端 Web 开发服务器：

```bash
pnpm dev
```

生产构建：

```bash
pnpm tauri build
```

现有检查命令：

```bash
pnpm check             # 类型检查、前端测试、Rust fmt/check/Clippy/测试
pnpm build             # 前端生产构建
```

`pnpm check` 已在 `.github/workflows/quality.yml` 中配置为 Windows CI 门禁。执行检查会生成 `src-tauri/target/`，`pnpm build` 会生成 `dist/`；两者均已被 Git 忽略。

## Windows 用户发布

普通用户请下载发布产物，而不是源码和 `Start_AI_API_Monitor.bat`。安装版或便携版均不要求 Node.js、pnpm、Rust、Cargo、Python 或 Git；发布说明、数据位置、更新预留和一键构建命令见 [docs/RELEASE.md](./docs/RELEASE.md)。

Windows 安装版已完成实际安装验收；安装后应用启动、Provider 管理与刷新、主题资源、托盘/悬浮窗、设置持久化、开机自启和数据隔离均可正常使用。

## 运行时数据

- SQLite 文件：Tauri 系统应用数据目录下的 `ai-api-monitor.db`。
- API Key：系统凭据库，service 为 `com.aiapimonitor.desktop`；SQLite 仅保存引用。
- Codex：不读取认证文件，仅执行 `codex login status` 检测公开登录状态。
- 前端主题预缓存：WebView `localStorage` 的 `ai-monitor-theme`；权威布局同时保存在 SQLite `settings` 表。

不要把数据库、系统凭据导出、`auth.json`、日志中的敏感响应或真实 Key 提交到 Git。

## 开发规范

### 前端

- 所有后端调用集中封装在 `src/api.ts`，页面不要直接散落 `invoke`。
- 前后端字段使用 camelCase 传输；修改 DTO 时同步检查 `src/types.ts` 与 Rust `#[serde(rename_all = "camelCase")]`。
- 未知的平台数据使用 `null`，不要用 `0` 代替未知余额、费用或当日 Token。
- 布局变更通过 `App` 的统一状态和持久化入口处理，避免多个组件分别写入。
- 保持 TypeScript `strict`、`noUnusedLocals`、`noUnusedParameters` 通过。

### Rust 后端

- 新 Provider 实现 `ProviderAdapter`，并在 `ProviderManager::new()` 显式注册；未注册实现不会出现在 UI。
- API URL 必须经过 `validate_provider_input`；远程地址只允许 HTTPS，本机 mock 可使用回环 HTTP。
- API Key 只能通过 `SecureStorage` 访问，禁止写入 SQLite、日志、错误文本或返回前端。
- 外部请求必须设置超时，并将平台错误转换为 `ProviderError`。
- 数据库结构变更必须新增递增迁移，保留幂等测试和旧版本迁移测试。
- 费用/余额进入数据库前应校验非负且有限；时间口径变更需同步历史查询和 UI 文案。

### 提交前建议检查

```bash
pnpm build
cd src-tauri
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

上述检查已写入 `package.json` 和 CI；Clippy 使用 `-D warnings`，任何警告都会使检查失败。

## 文件说明

| 路径 | 说明 |
|---|---|
| `src/App.tsx` | 应用根状态、页面与窗口模式 |
| `src/pages/Dashboard.tsx` | 刷新调度和 Widget 容器 |
| `src/pages/Settings.tsx` | Provider 与应用设置 |
| `src/components/TrendWidget.tsx` | 历史与预测展示 |
| `src/api.ts` / `src/types.ts` | 前后端命令边界与 DTO |
| `src-tauri/src/lib.rs` | Tauri 装配、托盘和窗口事件 |
| `src-tauri/src/commands.rs` | 命令、刷新、统计、提醒 |
| `src-tauri/src/settings.rs` | CRUD、校验和迁移 |
| `src-tauri/src/providers/` | 平台适配器 |
| `src-tauri/src/db/mod.rs` | SQLite schema 与迁移 |
| `src-tauri/src/storage.rs` | 系统凭据库 |
| `src-tauri/src/window_mode.rs` | Full/Mini/Ball 与几何持久化 |
| [`TEST_CASES.md`](./Markdown/Development/TEST_CASES.md) | 62 条手工/集成测试用例 |
| [`项目索引.md`](./Markdown/Development/项目索引.md) | Markdown 目录说明及完整结构、模块和资源索引 |
| [`优化建议.md`](./Markdown/TODO/优化建议.md) | 按优先级拆分的改进任务 |

## 新增 Provider 的最小流程

1. 在 `src-tauri/src/providers/` 新建适配器并实现 `ProviderAdapter`。
2. 为响应解析、非法值和 URL 契约添加 Rust 单元测试。
3. 在 `ProviderManager::new()` 注册类型。
4. 在 `Settings.tsx` 增加默认 Base URL、无 Key 规则或权限提示（如需要）。
5. 更新 `README.md`、`Markdown/Development/TEST_CASES.md` 和项目索引。
6. 使用 HTTP mock 验证成功、401/403、429、超时、非 JSON 与字段缺失。

## 通用自定义 API（custom）

`custom` 类型是通用自定义 API 适配器，用于接入任意返回余额/额度/用量/重置时间的 HTTPS 接口：

- 请求：方法（GET/POST）、URL、Query（结构化编码）、Headers、JSON Body（仅 POST）。
- 认证：Bearer Token、API Key Header、Basic Auth、无认证、自定义 Header。
- 响应：JSON 点路径映射 `remainingPath`/`totalPath`/`usedPath`/`resetTimePath`，支持嵌套对象与数组索引。
- 单位：Token、次数、金额、自定义。
- 余额：`remaining` 优先；否则 `total - used`；缺失字段保留未知，不用 0 伪装。

安全：Token/Key/密码/自定义 Header 值只经系统 keyring 存储，SQLite 仅存非敏感配置（`providers.custom_config`，schema V7）与不可逆的 `key_ref`；测试连接返回脱敏后的解析结果与响应结构预览。远程默认仅 HTTPS，本机回环 HTTP 仅用于测试连接。

## 后续计划

已完成数据库启动错误传播、趋势刷新联动、预测降级提示、OpenRouter URL 归一化、工具链固定、前端单元测试基线、CI、Provider HTTP mock 合约测试与数据库损坏安全恢复。下一阶段优先建设脱敏诊断日志导出、面向用户的数据库备份/恢复入口与模块拆分。长期目标是稳定 Provider 接口、显式能力模型、完整 DIY UI 和数据备份/恢复。

详细任务见 [`优化建议.md`](./Markdown/TODO/优化建议.md)。
