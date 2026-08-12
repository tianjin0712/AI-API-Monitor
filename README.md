# AI API Monitor

实时监控 **OpenAI / DeepSeek / Codex** 等 AI API 的 **Token 用量、余额、费用** 的跨平台桌面小工具。

> 定位：类似 GPU Monitor / Rainmeter / macOS Widget 的 AI 资源监控中心。
> 项目方案详见 [`mission.md`](./mission.md)。

## ✨ 功能

### V0.3 DIY UI

- ✅ 主题系统：亮/暗主题切换（标题栏按钮），持久化并启动恢复
- ✅ Widget 布局：账户列表 / 今日汇总 / 费用概览 三区块自由组合
- ✅ 编辑模式：Widget 拖拽排序 + 显示/隐藏（布局 JSON 持久化）

### V0.2 桌面能力

- ✅ 系统托盘：菜单（完整/Mini/小球模式、显示/隐藏/退出），左键单击切换窗口可见性
- ✅ 窗口状态机：Full（完整 Dashboard）/ Mini（紧凑条）/ Ball（悬浮小球），可拖动、点击展开
- ✅ Always On Top 置顶开关（持久化，启动自动恢复）
- ✅ 关闭按钮隐藏到托盘（桌面工具惯例），托盘「退出」真退出

### V0.1 基础版本

- ✅ 多 Provider 统一管理（DeepSeek、OpenAI、**Codex（ChatGPT 订阅额度）**；类型可扩展）
- ✅ Codex：复用 Codex CLI 登录态（`~/.codex/auth.json`）查询订阅剩余额度与重置时间
  （经 `chatgpt.com/backend-api/codex/wham/rate-limit-reset-credits`，需网络可达 chatgpt.com）
- ✅ Dashboard 卡片：余额、Token 用量、今日消耗、更新时间、进度条、逐账户失败状态
- ✅ Settings 页：添加 / 编辑 / 删除 API 账户
- ✅ 刷新策略：前台 10s / 后台 60s / 手动刷新 / 窗口聚焦立即刷新（单飞防重叠）
- ✅ API Key **加密保存**至系统凭据库（Windows Credential Manager / macOS Keychain），绝不明文落库，凭据引用唯一 ID
- ✅ SQLite 历史用量记账（`usage_history` 单日口径，为日报/周报/月报铺路）
- ✅ 无边框 + 透明窗口 + 深色毛玻璃 UI

## 🧱 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | [Tauri 2](https://tauri.app)（Rust 2.x / WebView2 / 内置托盘） |
| 前端 | React 19 + TypeScript + Vite 7 + Tailwind CSS 4 |
| 后端 | Rust（rusqlite / reqwest / keyring / tokio / chrono / uuid） |
| 数据 | SQLite（WAL，版本化迁移） |

## 📁 目录结构

```
├── src/                  # 前端（React + TS）
│   ├── components/       #   TitleBar / ProviderCard / MiniBall
│   ├── pages/            #   Dashboard / Settings
│   ├── api.ts            #   invoke 封装
│   └── types.ts          #   前后端共享类型
├── src-tauri/            # 后端（Rust）
│   ├── src/
│   │   ├── db/           #   SQLite 连接 + 迁移
│   │   ├── providers/    #   Provider 抽象 + DeepSeek/OpenAI/Codex 适配器
│   │   ├── storage.rs    #   keyring 安全存储（uuid 凭据引用）
│   │   ├── settings.rs   #   Provider CRUD + 设置
│   │   ├── commands.rs   #   Tauri commands
│   │   └── window_mode.rs#   Full/Mini/Ball 窗口状态机 + 置顶
│   └── tauri.conf.json
└── mission.md            # 项目方案文档
```

## 🚀 开发

```bash
pnpm install            # 安装依赖（含 @tauri-apps/cli）
pnpm tauri dev          # 开发模式（热更新）
pnpm tauri build        # 打包发布

cargo test              # 后端单元测试（src-tauri 目录下）
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

> 类型需在 `ProviderManager` 注册（白名单校验），URL 必须 HTTPS（本机回环除外）。
> 特例：`codex` 类型无需 API Key——自动复用 Codex CLI 登录态（`~/.codex/auth.json`），
> 前端表单会隐藏 Key 输入（见 `NO_API_KEY_TYPES`）。

## 🔐 安全

- API Key 仅存于系统凭据库（uuid 唯一引用），SQLite 只保存引用，前端不可见
- 凭据写入/数据库操作带失败补偿回滚
- 刷新间隔后端最终校验（前台 10–3600s / 后台 60–3600s / 后台 ≥ 前台）
- 数据库位于系统 app data 目录（`com.aiapimonitor.desktop`）

## 🗺️ 路线图

- V0.4 Claude / Gemini / OpenRouter / SiliconFlow
- V0.5 Token 历史、消耗曲线、费用与耗尽时间预测、额度预警
