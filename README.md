# AI API Monitor

实时监控 **OpenAI / DeepSeek / Codex / OpenRouter / Claude / SiliconFlow** 等 AI API 的 **Token 用量、余额、费用** 的跨平台桌面小工具。

> 定位：类似 GPU Monitor / Rainmeter / macOS Widget 的 AI 资源监控中心。
> 项目方案详见 [`mission.md`](./mission.md)。

## ✨ 功能

### V0.5 高级统计（V0.5-alpha）

- ✅ 消耗趋势：SVG 折线图（Token / 费用切换 + 账户下拉），基于当日 Token（`today_tokens`）与真实费用样本
- ✅ 费用预测：近 N 天有效费用样本日均 → 预计剩余天数 / 预计耗尽日期（含样本数与覆盖天数说明）
- ✅ 自动提醒：额度 <30% 黄色 / <10% 红色（剩余百分比）；余额型平台按预测剩余天数兜底（<7 天黄 / <3 天红），系统通知
- ⚠️ 已知限制：提醒仅对能提供剩余百分比或可预测的平台有效；历史趋势至少需多日刷新数据；系统通知首次使用需授权

### V0.4 更多平台

- ✅ **OpenRouter**：余额（key 剩余额度）+ 今日/月度费用 + 重置时间（`/api/v1/key`）
- ✅ **SiliconFlow（硅基流动）**：余额查询（`/user/info`；端点无官方文档页，实验性）
- ✅ **Claude (Anthropic)**：组织级 Usage & Cost API（需 **Admin Key**，个人账户不可用；后付费无余额，仅用量/费用）
- ❌ **Gemini**：官方无公开余额/用量查询端点（仅 AI Studio Billing 页），**暂不可添加**，请到 aistudio.google.com 查看

### V0.3 DIY UI（V0.3-alpha：排序 / 隐藏 / 双主题）

- ✅ 主题系统：亮/暗主题切换（标题栏按钮），持久化并启动恢复
- ✅ Widget 布局：账户列表 / 今日汇总 / 费用概览 三区块自由组合
- ✅ 编辑模式：Widget 拖拽排序 + 显示/隐藏（布局 JSON 持久化）

> 注：缩放、透明度、圆角、字体/颜色等完整 DIY 能力属后续迭代（见路线图）。

### V0.2 桌面能力

- ✅ 系统托盘：菜单（完整/Mini/小球模式、显示/隐藏/退出），左键单击切换窗口可见性
- ✅ 窗口状态机：Full（完整 Dashboard）/ Mini（紧凑条）/ Ball（悬浮小球），可拖动、点击展开
- ✅ Always On Top 置顶开关（持久化，启动自动恢复）
- ✅ 关闭按钮隐藏到托盘（桌面工具惯例），托盘「退出」真退出

### V0.1 基础版本

- ✅ 多 Provider 统一管理（DeepSeek、OpenAI、**Codex（ChatGPT 订阅额度）**；类型可扩展）
- ✅ Codex：仅通过 `codex login status` 检测公开登录状态，不读取认证文件、Cookie 或 Token
  - ⚠️ **实验性**：依赖 codex-cli 0.146.0 内部接口，OpenAI 未公开承诺该端点稳定性；
    CLI 升级或服务端调整可能导致失效。Base URL 固定官方地址不可修改（防止本机凭证泄露）。
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
│   │   ├── providers/    #   Provider 抽象 + 7 平台适配器（deepseek/openai/codex/openrouter/siliconflow/claude/gemini）
│   │   ├── storage.rs    #   keyring 安全存储（uuid 凭据引用）
│   │   ├── settings.rs   #   Provider CRUD + 设置
│   │   ├── commands.rs   #   Tauri commands
│   │   └── window_mode.rs#   Full/Mini/Ball 窗口状态机 + 置顶
│   └── tauri.conf.json
└── Markdown/Project/mission.md # 项目方案文档
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

> 类型需在 `ProviderManager` 注册（白名单校验），URL 必须使用 HTTPS。
> 特例：`codex` 类型无需 API Key——仅执行 Codex CLI 的公开登录状态检查，
> 前端表单会隐藏 Key 输入（见 `NO_API_KEY_TYPES`）。

## 🔐 安全

- API Key 仅存于系统凭据库（uuid 唯一引用），SQLite 只保存引用，前端不可见
- 凭据写入/数据库操作带失败补偿回滚
- 刷新间隔后端最终校验（前台 10–3600s / 后台 60–3600s / 后台 ≥ 前台）
- 数据库位于系统 app data 目录（`com.aiapimonitor.desktop`）

## 📦 发布（V1.0-alpha）

自动更新需要发布者在构建时配置签名与更新源（`tauri-plugin-updater` 已集成）：

> 当前仓库的 `pubkey` 与 `endpoints` 为占位空值，因此自动更新仅为集成骨架；发布包前必须完成下面的配置。

```bash
# 1. 生成更新签名密钥（仅一次，妥善保管）
npx tauri signer generate -w ~/.tauri/myapp.key

# 2. 在 src-tauri/tauri.conf.json 的 app.updater 段填写：
#    "endpoints": ["https://your-host/updates/{{target}}/{{arch}}/{{current_version}}"],
#    "pubkey": "<上面生成的公钥>"

# 3. 打包发布（Windows 生成 NSIS/MSI 安装包）
pnpm tauri build

# 4. 将安装包与生成的 .sig 签名文件上传到更新源服务器
```

未配置更新源时，应用内「检查更新」会提示"更新器未配置"；配置后即可正常检查/下载/安装。

### 平台

- Windows：NSIS 安装包（默认）与 MSI
- macOS：DMG（需在 macOS 上执行构建并配置签名证书）
- 发布 CI 参考 `.github/workflows/quality.yml`（质量检查）；正式发布流水线可按需扩展

## 🗺️ 路线图

- 完整 DIY UI（Widget 缩放 / 透明 / 圆角 / 字体 / 颜色 / 自由定位）
- Codex Provider 官方化（依赖稳定公开接口替代内部端点）
- 前端组件自动化测试与 HTTP mock 集成测试
- 真实账户端到端验证记录
