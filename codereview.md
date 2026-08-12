# AI API Monitor 代码审查

审查日期：2026-08-12  
审查范围：当前工作目录的 React/TypeScript 前端、Tauri/Rust 后端、SQLite 与 Provider 实现；以 `mission.md` 的 V0.1 目标为验收基准。

## 结论

项目已完成可编译的前端界面、Provider 管理、SQLite 初始化、安全凭据库接入，以及 DeepSeek 余额查询的主体代码；但不能认定 V0.1 MVP 已完成。OpenAI 用量查询实现与当前官方 API 不兼容，刷新失败被静默吞掉，且密钥引用会因同名账户冲突，均会直接影响用户数据的正确性或账户可用性。

前端生产构建（`pnpm build`）通过。项目没有自动化测试；当前环境未安装 Rust/Cargo，故未能运行 `cargo check` 或验证 Tauri 后端和桌面端实际行为。

## V0.1 任务完成情况

| 任务 | 状态 | 证据与说明 |
| --- | --- | --- |
| Tauri + React + TypeScript 项目初始化 | 已完成 | 项目结构、Vite 与 Tauri 配置齐全。 |
| 无边框、透明主窗口 | 已完成 | `tauri.conf.json` 配置了 `decorations: false` 与 `transparent: true`；前端有自定义标题栏。 |
| SQLite 数据库 | 已完成 | 启用 WAL、外键与两版 schema migration。 |
| Provider 架构 | 基本完成 | 有 `ProviderAdapter` trait 与注册表；目前只注册 DeepSeek、OpenAI。 |
| DeepSeek 余额 | 基本完成 | 已调用 `/user/balance` 并读取首个余额项；未覆盖多币种余额、不可用账户状态。 |
| OpenAI Usage | 未完成 | 请求路径和响应模型均不符合当前官方接口，实际账户无法得到正确用量。 |
| Provider 增删改查 | 基本完成 | 功能通路齐全，但同名账户会共享/覆盖凭据，且失败时有残留凭据。 |
| Dashboard 显示 | 基本完成 | 可显示卡片和刷新状态；失败账户的错误未展示，统计口径不正确。 |
| 前台 10 秒、后台 60 秒、手动/聚焦刷新 | 部分完成 | 前端轮询已写；未实现后端调度、系统唤醒刷新及可靠的后台状态判断。 |
| API Key 不落 SQLite | 基本完成 | 密钥写入系统凭据库，SQLite 仅存引用；引用设计仍有冲突风险。 |
| 历史用量记录 | 部分完成 | 有表和每日 UPSERT；写入的是滚动 30 天总量，不是当日增量，因此不能作为趋势/日报数据源。 |

## 问题清单

### P0：OpenAI 用量查询接口和响应结构错误

- 位置：`src-tauri/src/providers/openai.rs`
- 当前代码请求 `<baseUrl>/usage?start_time=...`，并将每个 `data` 元素直接解析为 `result.input_tokens` 等字段。
- 当前 OpenAI 官方接口为 `GET /organization/usage/completions`，费用为 `GET /organization/costs`；响应的 bucket 中包含 `results` 数组，而不是当前代码的单个 `result`。官方文档同时将其列在 Administration/Organization Usage API 下，接入前还应明确要求用户配置具备该权限的密钥。[OpenAI Usage API](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage)
- 后果：OpenAI Provider 会收到 404/403，或在响应解析后得到全零数据，核心 V0.1 功能不可用。
- 建议：以官方 `/organization/usage/completions` 与 `/organization/costs` 重写客户端；处理 `results` 聚合、分页和时间区间，并在 UI 明示所需密钥权限与不支持的账户类型。

### P0：同名 Provider 会覆盖同一条 API Key 凭据

- 位置：`src-tauri/src/storage.rs` 的 `account_for`，以及 `src-tauri/src/settings.rs` 的新增逻辑。
- 凭据 account 仅由 Provider 名称清洗后生成，例如两个名称均为“OpenAI 主账户”的记录都会使用 `provider_OpenAI_主账户`。数据库没有名称唯一约束，因此第二次添加会覆盖第一次的 key；删除其中任一记录还会删除另一记录仍在使用的 key。
- 后果：用户可正常创建两个账户，但刷新会使用错误密钥，或删除一个账户后另一个账户失效。
- 建议：先生成不可预测且唯一的 key id（UUID/数据库 id），以该 id 构造 keyring account；为迁移与删除保留明确的引用。不要以展示名称作为凭据主键。

### P1：刷新全部账户时静默丢弃失败，界面会展示陈旧或空数据

- 位置：`src-tauri/src/commands.rs` 的 `refresh_all`。
- 每个 Provider 的错误只写入后端标准错误输出，命令仍返回 `Ok(out)`。前端因此不会进入 `catch`，也不会看到失败原因；以前成功的数据仍停留在卡片中。
- 后果：密钥失效、网络故障或 OpenAI 403 时，用户无法判断余额/用量是否可信。
- 建议：返回按 Provider 对应的成功/失败结果（包含 `provider_id`、更新时间和可展示错误），或在全量刷新至少聚合失败并让前端显式标识“数据已过期/刷新失败”。

### P1：历史表记录的是 30 天累计数，不能用于日报、趋势或费用预测

- 位置：`src-tauri/src/providers/openai.rs`、`src-tauri/src/commands.rs` 的 `record_usage`。
- OpenAI 代码把最近 30 天 bucket 的 token 求和后写入当天 `usage_history.tokens`。每天 UPSERT 的是“截至当前的滚动 30 天总量”，而不是该日 token；`today_cost` 也从未填充。
- 后果：未来按该表绘制的日/周/月曲线会严重失真，预测也没有可信输入。
- 建议：以 UTC 或用户时区定义日边界，保存单日 bucket 的 token/cost；在同一天内按最新单日累计覆盖，或保存原始快照并计算差量。将滚动周期统计与历史日值分开建模。

### P1：凭据库与数据库操作不是原子操作，失败会留下错误状态

- 位置：`src-tauri/src/settings.rs` 的 `add_provider`、`update_provider`、`delete_provider`。
- 新增时先写 keyring、再写数据库，数据库写入失败会遗留凭据；更新时先更新数据库、再更新 keyring，后者失败会让展示配置与实际密钥状态不一致；删除时先删除数据库，并且忽略凭据删除失败。
- 后果：凭据残留、密钥无法追踪或账户配置已更新但实际请求仍使用旧 key。
- 建议：设计补偿逻辑：数据库失败后删除刚写入的 key；更新前读取旧状态并在后续失败时回滚；删除失败应记录待清理项或向用户报告。对数据库侧使用事务。

### P1：刷新调度存在重置与并发竞态，后台策略也不完整

- 位置：`src/pages/Dashboard.tsx`。
- 定时器 effect 依赖 `refreshingIds.size`。每次刷新开始/结束都会销毁并创建新的定时器；由于状态更新异步，手动刷新、聚焦刷新和 tick 仍可能重叠。`Math.max(5, intervalOf())` 的单位是毫秒，保护值实际为 5ms 而非 5 秒。
- 后果：短时间内可能重复请求 API，违反低频刷新目标并消耗额度；用户设置异常值时尤其明显。
- 建议：用 `useRef` 保存“正在刷新”和定时器；以单一调度循环保证一次只存在一个刷新任务；把最小值明确为 `5_000` 毫秒，并在前、后端都限制合理区间。用 Tauri 窗口事件/系统唤醒事件实现真正的前后台和唤醒刷新。

### P2：服务端未验证 Provider 类型与 API URL

- 位置：`src-tauri/src/settings.rs` 的 `add_provider` / `update_provider`。
- 仅检查非空，未验证 `provider_type` 已在注册表中，也未将 `api_url` 解析为允许的 HTTPS URL。前端限制不能替代 command 层验证。
- 后果：可保存永远不能刷新的类型或畸形 URL；作为桌面应用中的网络请求入口，也扩大了向本机/内网地址发送带认证请求的风险。
- 建议：command 层依据 `ProviderManager` 白名单验证类型；使用 URL 解析器校验 scheme，官方 Provider 固定可信 host，自定义 Provider 另设显式风险提示和限制策略。

### P2：刷新设置允许不合理的数值，前后端约束不一致

- 位置：`src/pages/Settings.tsx` 和 `src-tauri/src/commands.rs` 的 `set_refresh_settings`。
- HTML 的 `min` 不会阻止手工输入/调用，后端只拒绝 0，不拒绝 1 秒、超大值或前台大于后台等逻辑异常。
- 后果：用户可设置极高频请求，触发 API 限流或费用异常。
- 建议：后端作为最终边界，限定例如前台 10–3600 秒、后台 60–3600 秒，并要求后台间隔不小于前台；返回清晰校验错误。

### P2：`keyRef` 不必要地暴露给前端

- 位置：`ProviderConfig`（Rust）与 `src/types.ts`。
- 虽然 `key_ref` 不是 API Key，但它是系统凭据库中账户记录的定位信息，界面不需要它。
- 后果：增加了凭据元数据泄露与未来误用的攻击面。
- 建议：拆分数据库实体与前端 DTO，`list_providers`、新增和更新的返回值中移除 `keyRef`。

### P2：没有自动化测试，后端未在当前环境验证

- 位置：全项目。
- 未发现单元、集成或端到端测试。当前机器缺少 `cargo`，Tauri/Rust 代码也没有实际编译。
- 后果：Provider JSON 解析、迁移升级、凭据异常与刷新策略等高风险路径无法回归验证。
- 建议：至少补充 Rust 单元测试（迁移、key_ref 唯一性、OpenAI/DeepSeek fixture 解析、刷新结果）和前端组件/调度测试；在 CI 执行 `pnpm build`、`cargo check`、`cargo test`。

## 其他不合理或未达成项

- `mission.md` 和 `README.md` 的文本包裹了写作块标记及多余代码围栏，README 在常见渲染器中会出现乱码/格式异常；应清理为普通 UTF-8 Markdown。
- README 宣称“SQLite WAL、版本化迁移”“后台 60 秒刷新”等能力已完整可用，但缺少 Rust 实际构建与桌面端验证，且后台/唤醒调度没有后端实现，表述应降级为“已实现基础代码”。
- `ProviderType` 前端联合类型包含 `codex` 和 `custom`，后端注册表却只有 `deepseek`、`openai`，类型定义与实际功能不一致。
- `refresh_provider` 命令已经实现，但 Dashboard 没有单卡片刷新入口；批量刷新一个账户失败也会让所有卡片显示“刷新中”。
- DeepSeek 只读取 `balance_infos.first()`，没有处理账户不可用字段或多币种余额，展示精度和可解释性不足。
- 数据库连接使用全局 `Mutex<Connection>`；网络请求虽然在锁外，但所有数据库读写仍串行。V0.1 可接受，后续增加历史查询和后台任务时应考虑连接池或专用数据库线程。
- `csp` 配置为 `null`。桌面端若未来引入外部内容、富文本或插件，这会降低 XSS 防护；建议尽早设定最小可用 CSP。

## 建议修复顺序

1. 重写 OpenAI Provider（接口、认证要求、分页、响应聚合、费用）并以真实或 mock 响应测试。
2. 将 keyring 引用改为与 Provider 记录一一对应的随机 ID，并补齐新增/更新/删除失败补偿。
3. 让批量刷新返回逐账户状态，在 UI 显示错误与数据过期状态。
4. 修正历史数据口径与刷新调度的单飞控制、数值校验。
5. 安装 Rust 工具链后运行后端检查，并建立最小 CI 测试集。

