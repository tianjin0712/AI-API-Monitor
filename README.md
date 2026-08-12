# AI API Monitor

实时监控 **OpenAI / DeepSeek** 等 AI API 的 **Token 用量、余额、费用** 的跨平台桌面小工具。

> 定位：类似 GPU Monitor / Rainmeter / macOS Widget 的 AI 资源监控中心。
> 项目方案详见 [`mission.md`](./mission.md)。

## ✨ 功能（V0.1 MVP）

- ✅ 多 Provider 统一管理（DeepSeek、OpenAI；类型可扩展）
- ✅ Dashboard 卡片：余额、Token 用量、今日消耗、更新时间、进度条
- ✅ Settings 页：添加 / 编辑 / 删除 API 账户
- ✅ 刷新策略：前台 10s / 后台 60s / 手动刷新 / 窗口聚焦立即刷新
- ✅ API Key **加密保存**至系统凭据库（Windows Credential Manager / macOS Keychain），绝不明文落库
- ✅ SQLite 历史用量记账（`usage_history`，为日报/周报/月报铺路）
- ✅ 无边框 + 透明窗口 + 深色毛玻璃 UI

## 🧱 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | [Tauri 2](https://tauri.app)（Rust 2.x / WebView2） |
| 前端 | React 19 + TypeScript + Vite 7 + Tailwind CSS 4 |
| 后端 | Rust（rusqlite / reqwest / keyring / tokio / chrono） |
| 数据 | SQLite（WAL，版本化迁移） |

## 📁 目录结构

```
├── src/                  # 前端（React + TS）
│   ├── components/       #   TitleBar / ProviderCard
│   ├── pages/            #   Dashboard / Settings
│   ├── api.ts            #   invoke 封装
│   └── types.ts          #   前后端共享类型
├── src-tauri/            # 后端（Rust）
│   ├── src/
│   │   ├── db/           #   SQLite 连接 + 迁移
│   │   ├── providers/    #   Provider 抽象 + DeepSeek/OpenAI 适配器
│   │   ├── storage.rs    #   keyring 安全存储
│   │   ├── settings.rs   #   Provider CRUD + 设置
│   │   └── commands.rs   #   Tauri commands
│   └── tauri.conf.json
└── mission.md            # 项目方案文档
```

## 🚀 开发

```bash
pnpm install            # 安装依赖（含 @tauri-apps/cli）
pnpm tauri dev          # 开发模式（热更新）
pnpm tauri build        # 打包发布
```

> 国内镜像已配置：crates.io（中科大 sparse）位于 `~/.cargo/config.toml`，npm 走 npmmirror。

## 🔌 新增 Provider

实现 `ProviderAdapter` trait 并在 `src-tauri/src/providers/mod.rs` 的
`ProviderManager::new()` 注册即可，前端无需改动：

```rust
#[async_trait]
impl ProviderAdapter for MyProvider {
    async fn fetch_usage(&self, config: &ProviderConfig, api_key: &str)
        -> Result<ProviderUsage, ProviderError> { /* ... */ }
}
```

## 🔐 安全

- API Key 仅存于系统凭据库，SQLite 只保存 `key_ref` 引用
- 数据库位于系统 app data 目录（`com.aiapimonitor.desktop`）

## 🗺️ 路线图

- V0.2 系统托盘 / Always On Top / Mini 窗口 / 小球模式
- V0.3 DIY UI（Widget 拖拽、布局 JSON、主题系统）
- V0.4 Codex / Claude / Gemini / OpenRouter / SiliconFlow
- V0.5 Token 历史、消耗曲线、费用与耗尽时间预测、额度预警
