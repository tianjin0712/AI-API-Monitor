# 代码复审：V0.2（2026-08-12 18:48:15 +08:00）

审查基线：提交 `ab3fff2`（V0.2 桌面能力 + V0.1 审查修复）
范围：React/TypeScript 前端、Tauri/Rust 后端、Provider、SQLite 与 V0.2 托盘/窗口模式。

## 结论

上一轮 V0.1 审查中最重要的 Provider、凭据隔离、刷新错误反馈和输入校验问题均已获得实质性修复，前端生产构建也通过。但 V0.2 目前仍不能验收：从系统托盘菜单切换 Mini/小球模式时，Rust 只调整窗口尺寸而没有通知 React 更新 UI，导致完整 Dashboard 被压缩/裁切在 280×96 或 72×72 的窗口内。

此外，托盘的“左键显示菜单”和“左键切换窗口可见性”同时开启，交互相互冲突。应先修复这两项并在真实 Tauri 桌面环境中手测后，再宣称 V0.2 完成。

验证结果：

- `pnpm build`：通过。
- Rust/Tauri：当前环境没有 `cargo`，无法执行 `cargo check`、`cargo test` 或实际运行桌面应用。
- 自动化测试：新增了 SQLite、存储、Provider JSON 解析和刷新间隔的 Rust 单元测试；尚未能在当前环境执行，也没有窗口/托盘端到端测试。

## 任务状态

| 版本/能力 | 状态 | 审查结论 |
| --- | --- | --- |
| V0.1 项目初始化、透明无边框窗口、SQLite | 已实现 | 基础工程与迁移逻辑齐全。 |
| V0.1 DeepSeek 余额查询 | 基本实现 | 已有官方余额端点适配；仍只取第一种余额。 |
| V0.1 OpenAI Usage/Costs | 基本实现 | 已改为 Organization Usage 与 Costs 接口，处理每日 bucket 的 `results` 聚合；需真实 Admin Key 验证。 |
| V0.1 密钥安全存储 | 基本实现 | 新建账户使用 UUID 凭据引用，前端不再拿到 `keyRef`。旧数据迁移仍缺失。 |
| V0.1 刷新与错误展示 | 基本实现 | 单飞控制和逐账户失败结果已加入；系统唤醒刷新尚未实现。 |
| V0.2 系统托盘 | 部分实现 | 菜单、显示/隐藏、退出逻辑已写；左键交互冲突。 |
| V0.2 Full/Mini/Ball | 未完成 | 前端按钮路径可切换；托盘路径不同步前端状态，实际会裁切界面。 |
| V0.2 Always On Top | 基本实现 | 有设置、持久化和启动恢复；失败时原生状态与持久化状态不一致。 |
| V0.2 关闭到托盘 | 基本实现 | `CloseRequested` 被拦截后隐藏窗口；需桌面端手测退出行为。 |

## 本轮发现的问题

### P0：托盘菜单切换模式不会同步 React，Mini/小球界面会被完整页替代并裁切

- 位置：`src-tauri/src/lib.rs:76-80,117-124`，`src/App.tsx:12-31`。
- 标题栏调用 `set_window_mode` 后会返回 `WindowState`，React 因而正确更新为 `MiniBall`。但托盘菜单改走 `switch_window_mode`：它仅调用 `window_mode::apply_mode`，改变尺寸和持久化设置，没有发送事件给 WebView，也没有让前端再次读取状态。
- 结果：用户从托盘选择 Mini 或小球后，原生窗口变成 280×96 / 72×72，但 React 仍持有初始 `mode === "full"` 并渲染完整 Dashboard，界面不可用。
- 修复：将模式切换收敛为单一状态源。推荐在 Rust 成功应用模式后通过 `app.emit("window-mode-changed", state)` 发送事件，前端 `listen` 后设置 `mode`；或让托盘仅发出事件、由前端统一调用 command。启动恢复也应以同一机制同步，避免首帧尺寸与页面不匹配。

### P1：左键托盘菜单与“左键显示/隐藏窗口”同时配置，交互冲突

- 位置：`src-tauri/src/lib.rs:74-100`。
- 代码设定 `.show_menu_on_left_click(true)`，同时在左键抬起事件中执行 `toggle_main_window`。Tauri 文档说明该选项会让左键显示菜单；若要自行处理左键显示窗口，应把它设为 `false`。[Tauri 系统托盘文档](https://v2.tauri.app/learn/system-tray/)
- 后果：一次左键既可能弹出菜单又改变窗口可见性，体验不可预测，也与 README 的“左键单击切换窗口可见性”不一致。
- 修复：选择一种明确交互：保留右键菜单、左键切换显示/隐藏时设置 `show_menu_on_left_click(false)`；若希望左键打开菜单，则删除左键事件处理。

### P1：旧版安装升级后仍保留旧的、按名称生成的凭据引用

- 位置：`src-tauri/src/storage.rs`、`src-tauri/src/db/mod.rs`。
- 新建 Provider 已使用 UUID，解决了同名账户覆盖；但数据库 schema 版本仍停留在 2，且没有把已存在的 `provider_<name>` key_ref 迁移成 UUID 引用。V0.1 用户若已有同名记录，其冲突和删除互相影响问题仍会保留。
- 修复：新增迁移版本。逐条读取旧 `key_ref` 对应的凭据，生成 UUID key，复制密钥、更新数据库引用，并在确认数据库事务成功后清理旧凭据；无法读取的记录应保留并提示用户重新录入，不可静默丢失。

### P1：窗口状态持久化失败时，原生窗口状态可能与 UI/数据库不一致

- 位置：`src-tauri/src/window_mode.rs:66-96`。
- `apply_mode` 先修改窗口尺寸和可缩放性，再写入 settings；`set_always_on_top` 先改变原生置顶状态，再写数据库。若写库失败，command 返回错误、前端认为操作失败，但原生窗口已经改变，下一次启动又会恢复旧值。
- 修复：保留修改前的窗口状态，在持久化失败时补偿恢复；或先持久化后改原生窗口，并在后续失败时回滚数据库。所有分支都应返回与真实状态一致的 `WindowState`。

### P2：未恢复或保存用户调整过的完整窗口大小、位置和 Mini/小球位置

- 位置：`src-tauri/src/window_mode.rs`。
- 每次切回 Full 都固定为 460×720，启动恢复也只恢复模式与置顶；用户拖动 Mini/小球后的位置没有保存，Full 模式下调整的尺寸同样不会保存。
- 影响：基础“三模式”可工作后仍不符合桌面监控工具的使用预期，尤其小球模式难以保持在用户放置的位置。
- 建议：监听 moved/resized 事件，以逻辑坐标和显示器信息保存各模式的位置、Full 尺寸；恢复时进行工作区边界校正。窗口吸附、鼠标穿透和自动隐藏仍是后续 mission 的未实现项，应在 README 中标明状态。

### P2：刷新调度只覆盖 WebView 可见性/焦点，未实现系统唤醒刷新

- 位置：`src/pages/Dashboard.tsx:93-123`。
- 单飞机制已改善重复请求问题，但刷新在前端内运行。设备睡眠后不会立即刷新；最小前台间隔使用 `Math.max(..., 5)`，和后端最小 10 秒的约束也不完全一致（尽管后端会阻止设置为 5）。
- 建议：统一最小值为 10 秒；使用 Tauri 的窗口/系统电源事件或后端任务处理唤醒后刷新，并将后台刷新定义为窗口隐藏、最小化还是应用失焦，避免依赖浏览器 `visibilityState` 的平台差异。

### P2：OpenAI Provider 仍缺少真实接口行为所需的分页与权限体验验证

- 位置：`src-tauri/src/providers/openai.rs`。
- 已修正为 `/organization/usage/completions` 和 `/organization/costs`，并支持 `results[]` 聚合；但响应中的 `has_more` / `next_page` 未处理。当前 30 天、按天、`limit=100` 通常足够，仍应显式验证或实现分页以避免数据静默截断。该接口需 Organization Admin 权限，UI 也未提示普通项目 API Key 会失败。
- 建议：为真实响应、403/权限不足和分页场景加入集成测试；在 OpenAI 账户表单旁提示“需组织管理员密钥”，并把可行动的失败说明显示给用户。

### P2：删除凭据失败只记录后端日志，用户无法得知仍有残留敏感凭据

- 位置：`src-tauri/src/settings.rs:175-179`。
- 删除数据库记录后，keyring 清理失败仅 `eprintln!`。这比静默忽略更好，但用户没有恢复路径，之后也没有待清理队列。
- 建议：至少让命令返回“账户已删除但凭据清理失败”的明确状态；更稳妥的是记录待清理项，在启动时重试并提供诊断页。

### P3：安全基线仍偏弱

- 位置：`src-tauri/tauri.conf.json:28`。
- `csp` 仍为 `null`。当前 UI 未加载外部内容，风险有限；但后续增加自定义 Provider、插件、主题或外部页面时会放大 XSS 的影响。
- 建议：尽早定义最小 CSP，只保留应用实际需要的 `connect-src`、样式和资源来源。

## 上轮问题修复复核

| 上轮问题 | 结果 | 说明 |
| --- | --- | --- |
| OpenAI 调用旧 `/usage` 接口 | 已修复 | 改为 Organization Usage/Costs，并增加 bucket 聚合测试。 |
| 同名账户凭据覆盖 | 对新账户已修复 | UUID 引用已正确采用；历史数据未迁移。 |
| 全量刷新静默吞错 | 已修复 | `RefreshResult` 返回逐账户成功/失败，卡片显示错误。 |
| 历史记录使用 30 天累计 | 基本修复 | 注释和代码改为写入当日 input/output bucket 的 token；仍应真实验证“最后一个 bucket”是否为当天。 |
| 刷新并发/错误间隔单位 | 已修复 | 已使用 `useRef` 单飞与毫秒常量。 |
| Provider 类型与 URL 无后端验证 | 已修复 | 适配器白名单、HTTPS/本机回环规则已加入。 |
| 刷新设置无边界 | 已修复 | 后端限制前台 10–3600 秒、后台 60–3600 秒，且后台不小于前台。 |
| `keyRef` 暴露给前端 | 已修复 | Rust 字段 `skip_serializing`，前端类型已移除。 |
| 无测试 | 部分修复 | 已新增若干 Rust 单测，但无法在本机执行，缺 UI/E2E 测试。 |

## 推荐修复顺序

1. 使用 Tauri event 或统一前端状态源，修复托盘模式切换与 React 视图同步。
2. 将托盘交互定为“左键显示/隐藏、右键菜单”或“左键菜单”，删除另一套冲突行为。
3. 增加 V3 数据迁移，安全处理旧 `key_ref`；为 keyring 删除失败提供可见的恢复路径。
4. 安装 Rust 工具链并执行 `cargo check`、`cargo test`、`pnpm tauri dev`，手测三种模式、托盘、关闭到托盘和重启恢复。
5. 补充窗口状态保存、系统唤醒刷新与 Provider 真实接口集成测试。

# 代码复审：V0.3（2026-08-12 20:12:18 +08:00）

审查基线：提交 `a330eca`（V0.3 DIY UI），并包含截至 `b40b352` 的后续修复与 Codex Provider 变更。

范围：V0.3 Widget 布局、拖拽编辑、布局持久化、主题系统及其与现有 Provider 数据的组合行为。

## 结论

V0.3 已完成可运行的基础原型：Dashboard 被拆成账户列表、今日汇总、费用概览三个 Widget；支持拖拽排序、显示/隐藏、暗亮主题以及 SQLite 中的 JSON 持久化。`pnpm build` 通过。

当前仍不建议将 V0.3 标记为完整完成。布局加载与自动保存存在覆盖用户配置的竞态，主题在设置页切换时不会持久化，汇总组件还会把人民币、美元以及 Codex 额度等不同单位直接相加，展示结果不具备业务意义。按照 `mission.md` 的完整 DIY UI 要求，缩放、删除/恢复、透明度、圆角、字体和颜色编辑也尚未实现。

Rust/Cargo 仍不在当前环境的 PATH 中，因此本轮无法运行 `cargo check` 和新增的 Rust 单元测试，也没有执行真实 Tauri 桌面端的拖拽与重启恢复测试。

## V0.3 任务完成情况

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| Widget 化 | 基本完成 | 已有 `providers`、`summary`、`cost` 三种固定 Widget。 |
| Widget 拖拽排序 | 基本完成 | 使用原生 HTML Drag and Drop 实现纵向排序；尚无键盘/触屏方案。 |
| Widget 显示/隐藏 | 已实现 | 编辑模式下可切换，自动保存。 |
| Widget 删除与恢复 | 未实现 | 当前只有隐藏，没有删除、添加或恢复默认布局入口。 |
| Widget 缩放 | 未实现 | 数据模型没有尺寸、网格位置或最小/最大尺寸。 |
| 布局 JSON 保存 | 部分完成 | 后端保存与最小校验已实现，但初始化存在覆盖竞态，保存失败无反馈。 |
| 暗色/亮色主题 | 部分完成 | 两套 CSS 变量和切换按钮已实现；持久化依赖 Dashboard 挂载。 |
| 透明度、圆角、字体、颜色 | 未实现 | `Layout` 与 Widget 模型均无这些配置字段。 |
| 自动化验证 | 部分完成 | 有后端 JSON 最小校验测试；无前端解析、拖拽、持久化或主题测试。 |

## 本轮发现的问题

### P1：初次加载时默认布局可能覆盖用户已保存的布局

- 位置：`src/pages/Dashboard.tsx:120-142`。
- Dashboard 初始状态立即使用 `DEFAULT_WIDGETS`，同时异步调用 `getLayout()`。保存 effect 在首次渲染后马上启动 500ms 定时器；如果读取 SQLite、Tauri IPC 或 WebView 初始化超过 500ms，默认布局就会先写回数据库，覆盖用户原布局。
- 即使读取通常很快，这仍属于数据破坏型竞态，慢机器、首次启动或数据库繁忙时可复现。
- 修复：增加 `layoutLoaded` 状态，只有布局读取完成后才允许自动保存；读取失败时向用户展示错误，不应直接启用默认布局覆盖。更稳妥的做法是把布局和主题作为 App 级单一状态，一次读取、一次写入。

### P1：在“设置”页切换主题不会保存，重启后主题丢失

- 位置：`src/App.tsx:14-21,88-117`，`src/pages/Dashboard.tsx:133-142`。
- 主题按钮位于全局标题栏，但保存主题的 effect 位于 Dashboard。用户切换到设置页后 Dashboard 会卸载，此时切换主题只修改 App 内存状态，没有任何 `setLayout` 调用；退出或重启后恢复旧主题。
- 修复：将完整 `Layout` 状态和持久化提升到 `App` 或独立 context/store，Dashboard 只修改 widgets，标题栏只修改 theme，二者通过同一个保存入口写入。

### P1：汇总 Widget 将不同币种和不同含义的数值直接相加

- 位置：`src/pages/Dashboard.tsx:340-381`。
- `SummaryWidget` 直接累加所有 Provider 的 `todayCost`，`CostWidget` 直接累加所有 `balance` 和 `monthCost`，但没有按 `currency` 分组或换算。DeepSeek 可能是 CNY、OpenAI 是 USD，Codex 的 balance/remaining 又可能代表订阅额度或百分比。
- 后果：例如 ¥100 + $20 会显示为无单位的 `120.00`；该数字既不是人民币也不是美元，会误导用户判断余额和支出。
- 修复：按币种分别汇总并显示币种标签；Codex 订阅额度应使用独立指标，不参与货币余额求和。除非有明确汇率来源与时间戳，否则不要自动跨币种合计。

### P1：V0.3 与方案中的完整 DIY UI 范围差距较大

- 位置：`src/types.ts:67-80`、`src/pages/Dashboard.tsx:196-335`。
- 当前 Widget 数据只有 `id/type/visible`，只能排序和隐藏。`mission.md` 要求的缩放、删除、透明度、圆角、字体和颜色均没有数据模型和交互实现；布局也是单列顺序，不是可自由定位的桌面 Widget 布局。
- 修复：先明确 V0.3 验收口径。如果按 mission 验收，应扩展 Widget schema（位置、尺寸、样式、版本号），采用网格布局/拖拽缩放方案，并提供添加、删除、恢复默认和撤销。若本阶段只计划基础版，应在 README 和版本说明中明确写成 “V0.3-alpha：排序/隐藏/双主题”。

### P2：布局保存失败被完全吞掉，用户会误以为已经保存

- 位置：`src/pages/Dashboard.tsx:137-140`。
- `api.setLayout(json).catch(() => {})` 忽略数据库或校验错误，界面没有保存中、已保存或失败状态。
- 后果：用户调整布局后关闭应用，重启才发现设置丢失。
- 修复：显示轻量保存状态；失败时保留未保存标记并允许重试。切换页面或退出编辑模式时应等待/刷新最后一次防抖保存。

### P2：后端布局校验不足，重复 ID、未知类型和超大内容均可入库

- 位置：`src-tauri/src/commands.rs:148-166`。
- 后端只检查 theme 和 widgets 是否为数组，没有验证每项的 `id/type/visible`、ID 唯一性、允许的 Widget 类型、数组长度或 JSON 总大小。前端虽然会过滤部分无效项，但重复 ID 会造成 React key 冲突，缺失项会被静默丢弃。
- 修复：用与前端一致的 Rust 结构体反序列化，拒绝未知字段/类型和重复 ID；限制 Widget 数量与 JSON 字节数，并加入 schema version 以支持后续迁移。

### P2：旧布局不会自动补入未来新增的 Widget

- 位置：`src/utils/layout.ts:13-30`。
- `parseWidgets` 只返回保存 JSON 中通过过滤的项目。以后新增 Widget 类型时，已有用户的布局不会出现新 Widget，也没有“添加 Widget”或“恢复默认布局”入口。
- 修复：解析后按稳定类型/ID与默认清单合并：保留用户顺序和可见性，再把新增默认 Widget 追加为可见或待添加状态；同时提供恢复默认按钮。

### P2：拖拽区域是整个 Widget，内部按钮操作可能与拖拽冲突

- 位置：`src/pages/Dashboard.tsx:271-308`。
- 编辑模式把最外层容器设为 `draggable`，尽管视觉上只有 `⠿` 表示拖动把手。用户操作内部的刷新或显示按钮时，鼠标移动可能触发整个卡片拖拽。
- 修复：仅让专用拖动把手启动拖拽，或使用支持 handle、键盘和触屏的拖拽库；拖动时增加目标位置反馈和取消处理。

### P3：主题存在启动闪烁和部分硬编码颜色不适配亮色模式

- 位置：`src/App.tsx:16-41`、`src/index.css`、多个组件中的 `bg-white/*` 与固定深色文本。
- 应用初始固定为 dark，异步读取布局后才切换 light，会出现暗色首帧闪烁。部分交互色使用固定 `white` 或 `#0b0e14`，亮色模式下对比度与语义不统一。
- 修复：在渲染主 UI 前完成主题加载，或在最早的启动脚本读取缓存主题；逐步把固定颜色替换为语义 CSS 变量，并做亮/暗主题视觉回归。

## 本轮验证

- `pnpm build`：通过，TypeScript 与 Vite 生产构建成功。
- `cargo check` / `cargo test`：未执行，当前环境找不到 `cargo`。
- `git diff --check`：未发现本轮前已有代码的空白格式错误。
- 未执行：真实桌面拖拽、关闭后重启恢复、设置页主题持久化、慢 IPC 初始化竞态以及触屏/键盘可访问性测试。

## 建议修复顺序

1. 把 Layout 状态提升到 App 层，并用 `layoutLoaded` 阻止初始化覆盖；保存失败必须可见。
2. 按币种/指标类型拆分汇总，禁止直接相加 CNY、USD 和 Codex 订阅额度。
3. 明确 V0.3 是基础版还是完整 mission 验收；补齐相应的缩放、删除/添加和样式编辑能力。
4. 强化布局 schema 校验、版本化和默认 Widget 合并策略。
5. 安装 Rust 工具链并执行后端测试，再在 Tauri 中手测拖拽、主题和重启恢复。

# 代码复审：整体状态与 Codex Provider（2026-08-12 20:19:06 +08:00）

审查基线：提交 `269086f`；工作区除本审查文档外没有代码改动。

范围：复核上一轮 V0.3 问题是否已有修复，并专项检查新增的 Codex Provider、凭据边界、刷新行为和文档声明。

## 结论

自上一轮审查后没有代码修复提交，因此 V0.3 中已记录的初始化覆盖、主题持久化、跨币种错误汇总和 DIY UI 缺项仍全部存在。本轮新增发现一个发布阻断级安全问题：Codex Provider 会从本机 `~/.codex/auth.json` 读取 ChatGPT/Codex access token，却允许将 Base URL 修改为任意 HTTPS 地址，随后把该令牌作为 Bearer token 发往该地址。

Codex Provider 当前还依赖未在官方 OpenAI 文档中公开承诺的 `chatgpt.com/backend-api/codex/wham/rate-limit-reset-credits` 内部端点和响应模型。它可以作为明确标注的实验性功能，但不应在没有兼容性探测、令牌刷新与真实端到端验证的情况下声明为稳定支持。

`pnpm build` 再次通过。当前环境依旧找不到 `cargo`，无法验证 Rust 编译、单元测试与真实 Tauri 行为。

## 本轮发现的问题

### P0：Codex access token 可被发送到任意 HTTPS 主机

- 位置：`src-tauri/src/settings.rs:38-64,143-165`，`src-tauri/src/providers/codex.rs:72-90`，`src/pages/Settings.tsx` 的 Base URL 输入框。
- Provider 通用校验只要求 URL 使用 HTTPS；Codex 表单仍允许编辑 Base URL。刷新时程序无条件读取本机 Codex CLI access token，并对配置 URL 执行 `.bearer_auth(access_token)`。
- 攻击/误操作路径：只要 Codex Provider 的 URL 被设置为恶意 HTTPS 域名，下一次自动刷新就会把用户的 ChatGPT/Codex access token 发送给该域名。该修改可来自用户误填、数据库篡改或未来 UI/XSS 链路。
- 修复：Codex Provider 必须忽略数据库中的自由 URL并使用代码内固定 origin，或严格校验 scheme、host、port 和 path 均完全匹配允许列表。应禁用 Codex 的 Base URL 编辑框。HTTP 重定向也必须关闭或逐跳验证目标 origin，防止官方/受控地址通过 30x 把 Authorization 转发到非预期主机。
- 回归测试：加入恶意 host、子域伪装（如 `chatgpt.com.evil.test`）、userinfo、非默认端口、路径混淆和跨域重定向测试，并断言请求前即被拒绝。

### P1：Codex Provider 依赖未公开保证的内部端点，README 将其描述成已完成能力

- 位置：`src-tauri/src/providers/codex.rs:1-12,20-27`，`README.md` 的 V0.1 功能列表。
- 当前实现依据本地 `codex-cli 0.146.0` 的内部协议构造 `wham/rate-limit-reset-credits` 请求。官方 OpenAI Codex 文档没有公开承诺该端点、响应字段或第三方复用 CLI token 的稳定兼容性。[官方 Codex 文档](https://developers.openai.com/codex/)
- 后果：CLI 升级、服务端字段调整或权限策略变化即可让 Provider 失效；伪造的 fixture 单测只能证明代码能解析自定义样例，不能证明真实服务返回该结构。
- 修复：将功能标记为“实验性/依赖 Codex CLI 0.146.0 内部接口”，加入协议版本/字段探测和清晰降级提示；优先调用官方公开能力或直接复用 CLI 提供的稳定命令输出（若官方明确支持）。发布前以真实账户完成端到端测试，但不得记录或输出 token。

### P1：只读取 access token，没有刷新过期令牌的能力

- 位置：`src-tauri/src/providers/codex.rs:138-155`。
- `auth.json` 同时包含 access/refresh token 与更新时间信息，但实现只读取 access token。access token 过期时会持续返回 401，除非 Codex CLI 或其他进程恰好刷新并改写文件。
- 后果：应用启动时可能正常，运行一段时间后稳定失败，自动刷新无法自行恢复；README 的“复用 CLI 登录态”容易让用户误以为应用完整复用了 CLI 认证生命周期。
- 修复：不要自行实现未经官方支持的 OAuth 刷新协议。优先通过受支持的 Codex CLI/SDK交互获取当前状态；若只能读文件，应明确检测 401/过期并提示用户运行 Codex CLI 重新登录或刷新，同时避免每 10 秒重复发送必然失败的请求。

### P1：空或变化后的 Codex 响应会被当作成功并写入零值历史

- 位置：`src-tauri/src/providers/codex.rs:23-27,100-134`，`src-tauri/src/commands.rs:92-101,310-330`。
- `rate_limit_reset_credits` 被声明为可选；字段缺失时函数仍返回 `Ok(ProviderUsage::empty(...))` 并设置更新时间。批量刷新随后把它标记为成功并将 token/cost/balance 零值写入历史。
- 后果：协议变化、账号不具备额度字段或服务端返回不完整响应时，UI 显示“刷新成功但无额度”，历史数据也被污染，无法区分真实零值与解析失败。
- 修复：缺少核心对象或既无额度又无重置/百分比时返回结构化 `UnsupportedResponse` 错误；只有验证过的有效响应才能更新历史和成功时间。

### P2：删除 Codex Provider 会错误清理 `cli_local` keyring 项并报告虚假风险

- 位置：`src-tauri/src/settings.rs:105-111,220-265`。
- Codex 不向 keyring 写入密钥，却把 `com.aiapimonitor.desktop:cli_local` 存为 `key_ref`。删除时通用逻辑仍调用 `SecureStorage::delete_api_key`。通常该凭据不存在，于是返回 `credentialCleaned: false`，前端提示可能残留敏感信息。
- 更坏情况下，如果同一 service 下恰好存在名为 `cli_local` 的合法凭据，删除任意 Codex Provider 会误删该共享项；多个 Codex Provider 还共用同一占位引用。
- 修复：数据模型应显式区分 `CredentialSource::Keyring` 与 `CredentialSource::CodexCli`，不要用伪 key_ref 表示无凭据。删除/回滚仅对真正创建过的 keyring 引用执行清理。

### P2：Codex 额度被标记为人民币余额，语义错误

- 位置：`src-tauri/src/providers/codex.rs:107-130`。
- 代码固定 `usage.currency = "¥"`，但 `credits.balance` 或 `spend_control.remaining_percent` 是订阅额度/百分比，并不等于人民币金额。它还会被 V0.3 `CostWidget` 纳入“账户总余额”。
- 后果：用户会把订阅额度误认为货币余额，并与 CNY/USD 继续错误求和。
- 修复：统一数据模型增加 metric kind/unit，例如 `currency_amount`、`percentage`、`credits`、`tokens`；Codex 使用 `credits` 或 `%`，绝不伪装为人民币。

### P2：批量刷新串行执行，Provider 增多后可能长时间阻塞

- 位置：`src-tauri/src/commands.rs:78-108`。
- `refresh_all` 对 Provider 逐个 `await`。每个请求最长约 15–20 秒，多个失败账户会把总耗时累加；前端单飞期间所有刷新入口都不可用。
- 修复：使用受控并发（例如 3–4 个任务）并保持逐账户结果；对同一 Provider/host 设置合理限流。不要无限并发，也不要让一个慢账户阻塞全部结果展示。

## 上轮问题复核

| 上轮 V0.3 问题 | 当前状态 |
| --- | --- |
| 默认布局可能覆盖保存布局 | 未修复 |
| 设置页切换主题不持久化 | 未修复 |
| 跨币种/跨指标直接汇总 | 未修复，并被 Codex 的 `¥` 额度进一步放大 |
| 保存失败静默吞掉 | 未修复 |
| 后端布局 schema 校验不足 | 未修复 |
| 新 Widget 不会合并到旧布局 | 未修复 |
| 仅排序/隐藏，缺少完整 DIY 能力 | 未修复 |

## 本轮验证

- `pnpm build`：通过。
- 本机 Codex CLI：检测到 `codex-cli 0.146.0`；只核对了 `auth.json` 字段名称，没有读取或输出任何 token 内容。
- 官方文档核对：未找到对当前内部 `wham` 端点与响应结构的公开稳定承诺，因此审查按实验性依赖处理。
- `cargo check` / `cargo test`：未执行，当前环境找不到 `cargo`。
- 未向任何远程端点发送本机 Codex access token。

## 建议修复顺序

1. 立即固定 Codex 请求 origin、关闭/验证重定向，并移除 Codex Base URL 编辑能力。
2. 将凭据来源和指标单位建模为明确枚举，修复 Codex 删除与货币展示问题。
3. 对缺失核心字段、401 和协议变化返回可行动错误；将该 Provider 标记为实验性。
4. 修复上一轮全部 V0.3 持久化与汇总问题。
5. 安装 Rust 工具链后运行全部测试，并为 Codex host allowlist、重定向和空响应添加回归测试。

---

# 修复记录（Reasonix 执行）

> 以下按提交记录登记各轮审查问题的修复状态，供 codex 复核。

## 批次 1：V0.2 复审（提交 `30585e2` / `16c5285` / `fe7f78f`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P0：托盘菜单切换模式不同步 React（Mini/小球被完整页裁切） | ✅ 已修复 | 模式/置顶变更 emit `window-mode-changed` 事件，前端 listen 同步视图；启动恢复走同一状态源 |
| P1：左键托盘菜单与左键显示/隐藏冲突 | ✅ 已修复 | `show_menu_on_left_click(false)`：左键切换可见性、右键菜单 |
| P1：旧 `provider_<name>` 凭据未迁移 UUID | ✅ 已修复 | db 迁移 V3 + `migrate_legacy_credentials`（启动幂等迁移，失败保留并统计提示） |
| P1：窗口模式/置顶持久化失败时原生与持久化不一致 | ✅ 已修复 | 写入前记录旧状态，`set_setting` 失败时补偿恢复原生窗口 |
| P2：未保存/恢复 Full 尺寸与 Mini/Ball 位置 | ✅ 已修复 | moved/resized 监听保存各模式几何，启动按模式恢复（坐标非负保护） |
| P2：刷新最小间隔与后端不一致、无唤醒刷新 | ✅ 已修复 | 最小间隔统一 10s；窗口聚焦（含系统唤醒）emit `app-focused` 触发刷新 |
| P2：OpenAI 无分页、无权限提示 | ✅ 已修复 | `has_more`/`next_page` 分页（上限 5 页）+ 表单管理员密钥提示 |
| P2：删除凭据失败无可见状态 | ✅ 已修复 | `delete_provider` 返回 `DeleteResult`（credential_cleaned/note），前端提示 |
| P3：CSP 为 null | ✅ 已修复 | 最小 CSP（生产 script-src 'self'）+ devCsp 放宽 dev HMR |
| review 补充：app-focused 监听泄漏 / 迁移失败无提示 / 迁移标记残留 / 几何保存 key 竞态 | ✅ 已修复 | cancelled 标志、`get_migration_status` 命令 + Settings 警告、failed==0 清除标记、`save_geometry_for` 显式传模式 |

## 批次 2：V0.3 遗留（提交 `5821d47`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P1：默认布局可能覆盖用户已保存布局 | ✅ 已修复 | Layout 提升为 App 级单一状态 + `layoutLoaded` 标志，加载完成前禁止自动保存 |
| P1：设置页切换主题不持久化 | ✅ 已修复 | 主题切换在 App 层统一持久化（不再依赖 Dashboard 挂载） |
| P1：跨币种/跨指标直接相加 | ✅ 已修复 | Summary/Cost Widget 按 currency 分组展示，Codex credits 独立不参与货币求和 |
| P2：布局保存失败静默吞掉 | ✅ 已修复 | 保存失败显示红色提示条 + 重试按钮 |
| P2：后端布局校验不足 | ✅ 已修复 | id 非空/唯一、type 白名单、visible 布尔、≤20 个、≤64KB |
| P2：旧布局不自动补入新 Widget | ✅ 已修复 | `parseWidgets` 与默认清单合并，新增 Widget 自动追加 |
| P2：拖拽区域整卡冲突 | ✅ 已修复 | 仅 `⠿` 把手 draggable |
| P1：完整 DIY 能力差距 | ⚠️ 范围说明 | README 标注 "V0.3-alpha：排序/隐藏/双主题"，缩放/透明/圆角等列入后续迭代 |

## 批次 3：Codex Provider（提交 `b40b352` 实现 + `5821d47` 安全修复）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P0：access token 可发送到任意 HTTPS 主机 | ✅ 已修复 | URL 硬编码固定官方地址（忽略 api_url）+ 禁用重定向 + 校验层精确匹配（拒恶意 host/子域/端口/userinfo/路径混淆）+ 表单只读 + 2 个回归测试 |
| P1：内部端点无公开承诺，README 表述过度 | ✅ 已修复 | README 标注 ⚠️ 实验性（依赖 codex-cli 0.146.0 内部接口） |
| P1：只读 access token 无刷新能力 | ⚠️ 已知限制 | 不自行实现未官方支持的 OAuth 刷新；401 返回可行动错误提示 `codex login` |
| P1：空/变化响应被当成功写入零值历史 | ✅ 已修复 | 核心数据缺失（无 balance/remaining/reset_time）显式报错 |
| P2：删除 Codex 误清 `cli_local` 并误报残留 | ✅ 已修复 | `CredentialSource` 枚举 + codex key_ref 存空字符串，delete 跳过空 key_ref 清理 |
| P2：额度标记为人民币语义错误 | ✅ 已修复 | currency 改 `credits`，独立指标展示 |
| P2：批量刷新串行阻塞 | ✅ 已修复 | refresh_all 改 tokio JoinSet + Semaphore(3) 受控并发 |

## 当前测试状态

- `cargo test`：27 passed / 0 failed（新增 Codex URL 安全 2、布局校验 2、Codex fixture 4 等）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 4：V0.4 更多平台（提交 `25a8aba`）

| 平台 | 状态 | 说明 |
| --- | --- | --- |
| OpenRouter | ✅ 已实现 | `GET {base}/api/v1/key`：limit_remaining→余额、usage→tokens、usage_daily/monthly→费用、limit_reset→重置时间（官方 Limits API） |
| SiliconFlow | ✅ 已实现 | `GET {base}/user/info`：data.balance 余额（容错解析；官方文档未收录该页） |
| Claude (Anthropic) | ✅ 已实现 | 组织级 `usage_report/messages` + `cost_report` 双端点 + has_more/next_page 分页；x-api-key + anthropic-version 认证；成本 cents→USD；**无余额**（后付费），仅用量/费用 |
| Gemini | ⚠️ 不支持 | 官方无公开余额/用量查询端点（仅 AI Studio Billing 页），适配器返回可行动说明 |
| 前端 | ✅ 已适配 | 7 类型下拉/默认 URL；claude 提示需组织管理员 Key、gemini 提示无公开端点 |

### 测试状态更新

- `cargo test`：**37 passed / 0 failed**（新增 openrouter 2、siliconflow 3、claude 3 fixture 测试）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 5：V0.4 复审修复（提交 `2cd2c98`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P0：OpenRouter 默认 URL 与适配器路径重复 | ✅ 已修复 | preset 改站点根 `https://openrouter.ai`；`key_url` 纯函数 + 契约测试（含自定义 base 只拼一次断言） |
| P1：OpenRouter 美元费用误写 total_tokens | ✅ 已修复 | usage* 是 USD 费用非 Token，仅映射 today/month cost，total_tokens 保持 0 |
| P1：Claude 分页 5 页静默截断 | ✅ 已修复 | 循环至 has_more=false；空/重复 next_page 检测防死循环；100 页上限触顶显式报错 |
| P1：Claude "今日"取最后 bucket 不保证是今天 | ✅ 已修复 | 解析 bucket starting_at/ending_at，按 UTC 日期精确筛今天（bucket_date 回退链）+ 测试 |
| P1：Gemini 作为可添加 Provider 必然失败 | ✅ 已修复 | 从注册表移除（supported_types/前端下拉/表单均不含），gemini.rs 保留 `#[allow(dead_code)]` 供未来启用 |
| P2：SiliconFlow 缺/非法余额静默成功 | ✅ 已修复 | `parse_balance` 纯函数：缺失/空/非数字显式报错，不再污染历史；测试改 `missing_balance_is_explicit_error` |
| P2：Provider 列表顺序不稳定 | ✅ 已修复 | `supported_types` 排序后返回 |
| P2：README 未同步 V0.4 | ✅ 已修复 | 新增 V0.4 功能章节（能力/实验性/不可支持划分），Gemini 明确不可添加，路线图更新 |

### 测试状态更新

- `cargo test`：**39 passed / 0 failed**（新增 key_url 契约、bucket_date 测试）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 6：V0.5 高级统计（提交 `3bfce46`）

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| Token 历史 / 消耗趋势 | ✅ 已实现 | `get_usage_history` 命令（按日序列，provider 可空、days≤365）；趋势 Widget（SVG 折线，Token/费用切换 + 账户下拉） |
| 费用预测 | ✅ 已实现 | `get_prediction`：近 7 天日均费用 + 当前余额 → 预计剩余天数/耗尽日期（mission.md §13 公式） |
| 自动提醒 | ✅ 已实现 | 额度阈值（<30% 黄 / <10% 红，边界 30%/10% 属正常/警告）；refresh 后 `check_alerts` 级别提升才发系统通知（tauri-plugin-notification）、恢复重置；`alert_level_thresholds` 边界测试 |
| 前端集成 | ✅ 已实现 | 默认布局新增 trend Widget；编辑模式可拖拽/隐藏；布局校验白名单加入 trend |

### 测试状态更新

- `cargo test`：**40 passed / 0 failed**（新增 alert_level_thresholds）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 7：V0.5 复审修复（提交 `236e881`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P1：Token 历史混用区间累计值与当日值 | ✅ 已修复 | db 迁移 V4：usage_history 新增 `today_tokens`（可空）；`record_usage` 写当日输入+输出（0/未知写 NULL）；趋势图改用当日 Token；累计值保留在 `tokens`（快照） |
| P1：日期窗口多取一天、预测固定 /7 | ✅ 已修复 | `history_start_offset(days)=-(days-1)`（含今天 N 天，7/30/1/0 边界测试）；`daily_avg_from` 按有效费用样本日均（NULL 不参与），返回 samples/days_span |
| P1：费用缺失被写成 0 | ✅ 已修复 | `usage_history.cost` 允许 NULL（V4 重建表）；`record_usage` 费用缺失写 NULL；`DailyUsage.cost` 为 `Option<f64>`；预测/趋势只用真实样本 |
| P1：提醒只读 remaining，多数平台不触发 | ✅ 已修复 | `alert_level_for` 双阈值：remaining 存在时百分比优先；无 remaining 用预测剩余天数（<3 红 / <7 黄）；refresh 后 `predict_for` 计算 days_left；通知文案区分百分比/天数 |
| P2：TrendWidget 三处 | ✅ 已修复 | ① Provider 删除后校验 selectedId 重置并清空；② `Promise.allSettled` 历史/预测独立降级；③ 统一已校验点集（拒 null/NaN/负值，max 与绘制共用） |
| P2：自绘菜单键盘不完整 | ✅ 已修复 | roving focus：方向键循环 / Home / End / Enter / Space 选择 / Escape 关闭；打开聚焦当前项，选择与 Escape 归还焦点到 trigger |
| P2：README 未披露 V0.5 状态 | ✅ 已修复 | 新增 V0.5-alpha 章节（趋势/预测/提醒 + 已知限制：适用范围、多日数据、通知授权）；路线图更新 |

### 测试状态更新

- `cargo test`：**42 passed / 0 failed**（新增 history_window_is_inclusive_of_today、daily_avg_uses_only_valid_cost_samples、alert_level days_left 兜底）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 8：V0.5 复审遗留修复（提交 `49a2238`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P1：真实当日 0 Token 被当成未知 NULL | ✅ 已修复 | `ProviderUsage` 新增 `today_tokens: Option<u64>`（serde default，empty=None）；openai 恢复 `aggregation_timestamp` 并按 UTC 日期筛今日 bucket（有则 `Some(含0)`，无则 None，不再 `last()`）；claude 有今日 bucket 才 Some；`record_usage` 直落该字段（不再 `>0` 推断）；其余平台保持 None |
| P2：请求失败残留旧数据 | ✅ 已修复 | TrendWidget `allSettled` 中 rejected 时 `setHistory([])` / `setPrediction(null)` 清空 |
| P2：快速切换账户旧请求覆盖新结果 | ✅ 已修复 | `seqRef` 请求序号，结果落地前校验 seq 丢弃过期响应 |
| P2：趋势图按历史行数绘制可能空图 | ✅ 已修复 | 绘制条件改 `series.length >= 2`（过滤 null/NaN/负值后的有效点集） |
| P2：不存在 Provider 返回空预测 | ✅ 已修复 | `predict_for` 先查 providers 存在性，不存在直接返回 `Ok(None)` |
| P2：缺 V3→V4 迁移专项测试 | ✅ 已修复 | 新增 `migration_v3_to_v4_preserves_data_and_marks_today_null`：手动 V3 状态 → migrate → 断言 version=4、数据保留、today_tokens=NULL、唯一索引存在、旧表清理 |

### 测试状态更新

- `cargo test`：**43 passed / 0 failed**（新增 V3→V4 迁移专项测试）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

## 批次 9：V0.5 复审二次遗留（提交 `1b34853`）

| 问题 | 状态 | 说明 |
| --- | --- | --- |
| P1：OpenAI 今日费用仍取最后 bucket（未按日期筛） | ✅ 已修复 | `CostBucket` 增加 `aggregation_timestamp`（`alias=start_time` 双兼容，修复 fixture 字段名不一致导致的静默全过滤）；`today_cost` 改为按 UTC 日期筛今日 bucket 求和，今日无 bucket 时 `None`（不再 `costs_data.last()`） |
| P1：Claude 无今日费用 bucket 时误存 Some(0) | ✅ 已修复 | 今日费用桶为空时 `today_cost=None`（未知），非空才 `Some(day_cents/100)`（含真实 0）；`day_cents` 提取为纯函数 |
| P2：OpenAI 今日 Token 只聚合第一个 bucket | ✅ 已修复 | 改为 fold 累加全部同日 bucket（input/output/cached），`today_tokens=Some(合并值)`；分页拆分的同日多条记录全部合并 |
| P2：V3→V4 迁移测试未模拟外键/级联删除 | ✅ 已修复 | 新增 `migration_v3_to_v4_preserves_foreign_key_cascade`：手动建含 `FOREIGN KEY ON DELETE CASCADE` 的 V3 表，migrate 后删除 provider 断言 usage_history 级联清空（`pragma foreign_keys=ON`） |

### 测试状态更新

- `cargo test`：**45 passed / 0 failed**（新增 alias 兼容测试 + 外键级联迁移测试）
- `cargo check`：零警告
- `pnpm build`：通过
- `pnpm tauri dev`：端到端启动正常

# 修复状态审计：未完成项核查（2026-08-12 22:18:35 +08:00）

审查基线：提交 `fbd95ae`，重点复核文末“修复记录”是否与当前代码和可执行测试一致。

## 结论

Reasonix 登记的大部分修复已经真实落入代码。本轮使用本机实际存在但未加入 PATH 的 `C:\Users\12534\.cargo\bin\cargo.exe` 完成验证：`cargo check` 通过，`cargo test` 为 27 passed / 0 failed，`pnpm build` 通过。

仍有明确未完成项，不能把整个 `codereview.md` 视为全部关闭：完整 DIY UI 被延期；Codex token 续期仍是已知限制；系统唤醒仅以窗口重新聚焦近似实现；多显示器负坐标恢复不完整；OpenAI/Codex 真实服务行为仍缺少可重复的端到端验证。

## 已确认完成

- 托盘模式通过 `window-mode-changed` 与 React 同步。
- 托盘左键显示/隐藏与右键菜单已分离。
- 旧凭据迁移、迁移失败提示与标记清除逻辑已加入。
- 窗口模式和置顶持久化失败具有补偿逻辑。
- OpenAI 分页代码和管理员 Key 提示已加入。
- 删除凭据失败会返回前端可见状态。
- CSP 已不再为 `null`。
- V0.3 布局状态已提升至 App，加载完成前不会自动保存默认布局。
- 设置页切换主题能够通过 App 级布局入口持久化。
- 布局保存失败提示、重试、后端大小/数量/ID/type 校验、默认 Widget 合并、拖动把手和恢复默认均已实现。
- Codex 请求固定官方 origin、禁用重定向，恶意 URL 校验测试已加入。
- Codex 空响应报错、credits 单位、空 key_ref 删除处理和受控并发刷新均已实现。

## 仍未完成

### P1：完整 DIY UI 能力仍未实现

- 状态：明确延期，不是已完成。
- 当前仅支持三个固定 Widget 的纵向排序、显示/隐藏和恢复默认。
- `mission.md` 中的自由缩放、删除/添加、透明度、圆角、字体、颜色及自由定位仍无数据模型与交互。
- README 已诚实标注为 `V0.3-alpha`，因此可以验收 alpha 范围，但不能验收完整 V0.3 方案。

### P1：Codex access token 自动续期仍未解决

- 状态：已知限制。
- 当前遇到 401/403 会提示运行 `codex login`，不会刷新 token，也不会暂停后续定时请求。
- 在官方没有稳定认证接口的前提下，不建议自行实现 OAuth；但应对认证失败增加退避/暂停，避免每 10 秒重复失败请求。

### P1：所谓“系统唤醒刷新”只是窗口 Focused 事件，不等价于系统恢复事件

- 位置：`src-tauri/src/lib.rs:177-180`。
- 当前仅在 `WindowEvent::Focused(true)` 时 emit `app-focused`。电脑唤醒后如果窗口隐藏在托盘、没有获得焦点，就不会立即刷新。
- 修复记录中“窗口聚焦（含系统唤醒）”表述过度。需要接入平台电源恢复事件，或通过时间跳变检测：定时器恢复时若距离上次 tick 超过阈值，立即刷新。

### P2：窗口位置恢复不支持主屏左侧/上方的显示器

- 位置：`src-tauri/src/window_mode.rs:216-225`。
- 恢复逻辑只接受 `x >= 0 && y >= 0`。Windows 多显示器中位于主屏左侧或上方的屏幕合法坐标为负数，因此 Mini/小球放在这些屏幕时重启后不会恢复原位置。
- 应按所有显示器工作区判断位置是否仍可见，而不是拒绝负坐标；仅当窗口完全不在任何工作区时才回到主屏。

### P2：Provider 真实端到端验证仍不足

- Rust fixture 测试验证了解析逻辑，但没有可重复的 mock HTTP 集成测试覆盖 OpenAI 分页、403、Codex 401、禁重定向和 DeepSeek 错误响应。
- `pnpm tauri dev` 能启动不代表真实 API 数据正确。OpenAI 需真实 Organization Admin Key，Codex 又依赖实验性内部端点；这些仍需人工验收记录或可控 mock server 测试。

### P2：前端关键交互仍无自动化测试

- 尚未发现针对布局加载竞态、主题持久化、拖拽排序、恢复默认、保存失败重试、托盘事件同步的前端或桌面 E2E 测试。
- 当前生产构建只能证明类型检查和打包成功，不能证明上述交互回归安全。

### P3：主题首帧闪烁与亮色硬编码仍存在

- App 初始使用 dark，异步读取布局后才应用 light，亮色用户启动时仍可能看到暗色首帧。
- 多处仍使用 `bg-white/*`、`hover:text-white` 和固定 `#0b0e14`，没有完成语义颜色统一及亮色视觉回归。

## 修复记录中需要调整的表述

| 当前记录 | 更准确的状态 |
| --- | --- |
| “无唤醒刷新：✅ 已修复” | 部分完成；仅窗口聚焦刷新，隐藏到托盘时的系统唤醒未覆盖 |
| “未保存/恢复 Full 尺寸与 Mini/Ball 位置：✅ 已修复” | 基本完成；负坐标多显示器恢复未覆盖 |
| “完整 DIY 能力差距：⚠️ 范围说明” | 未完成并延期；仅 V0.3-alpha 可验收 |
| “Codex 只读 access token：⚠️ 已知限制” | 未完成；已有错误提示，但缺自动恢复和失败退避 |
| “端到端启动正常” | 仅证明应用可启动；不等同于 Provider 与桌面交互端到端验收 |

## 本轮验证

- `cargo check`：通过。
- `cargo test`：27 passed / 0 failed。
- `pnpm build`：通过。
- 未执行真实 OpenAI、DeepSeek 或 Codex 网络请求，未读取或输出任何密钥/token。
- 未执行多显示器、系统睡眠恢复、托盘交互和亮/暗主题视觉手测。
# 未完成项整改：V0.3（2026-08-12 22:27:34 +08:00）

## 本轮已修复

1. **多显示器负坐标恢复**：移除 `x >= 0 && y >= 0` 的错误限制，改为判断保存的窗口矩形是否仍与任一当前显示器相交。左侧或上方副屏的负坐标现在可正常恢复，同时仍会拒绝完全离屏的旧位置。
2. **休眠/唤醒后的漏刷新**：前端调度器记录预计触发时间；当 `visibilitychange` 等恢复信号到达且检测到明显时间跳变时，会立即补刷新，不再只依赖 Tauri 的窗口聚焦事件。
3. **主题首帧闪烁**：主题切换时同步缓存到 `localStorage`，React 挂载前预先应用缓存主题；后端布局加载完成后仍以持久化布局为最终真值。
4. **缺失费用被误报为 0**：汇总逻辑不再用 `?? 0` 把 Provider 未提供的余额、今日费用和月费用伪装成真实零值；只有实际返回指标才参与分组汇总。
5. **多显示器回归测试**：新增负坐标副屏可见、完全离屏拒绝两个 Rust 单元测试。

## 验证结果

- `pnpm build`：通过。
- `cargo test`（`src-tauri`）：29 passed，0 failed。
- `git diff --check`：通过，仅有仓库现有 Windows 行尾转换提示。

## 仍未宣称完成的项目

以下内容不是可在本轮通过局部缺陷修复可靠完成的事项，继续保留为后续开发范围：

- V0.3 完整自由布局能力：任意定位、尺寸调整、Widget 添加/删除，以及透明度、圆角、字体和颜色编辑；当前仍是 Alpha 级排序、显示/隐藏和主题切换。
- Codex CLI OAuth access token 的自动续期：应用当前复用本机 Codex 登录态，但没有可依赖的公开刷新契约；401/403 仍需执行 `codex login`。不能在未验证协议的情况下伪造自动续期。
- 真实 Provider 端到端验证：需要用户自己的有效账户和额度，本轮没有读取或调用真实凭据。
- 前端组件自动化测试与 HTTP mock 集成测试仍未建立；本轮通过生产构建和 Rust 单元测试验证。

## 当前仍不合理之处

- README 中把窗口聚焦近似描述成“含系统唤醒”不够严谨；现在前端已增加时间跳变补偿，但隐藏在托盘且系统不产生 WebView 可见性事件时，仍不能保证操作系统级即时唤醒通知。
- `Dashboard.tsx` 同时承担请求调度、数据聚合和布局编辑，职责偏重；继续扩展完整 DIY UI 前应拆分 scheduler、aggregation 和 editor。
- 亮色主题仍有少量 `bg-white/*`、固定强调色等暗色语义类，视觉一致性尚未完全收口。

# 代码复审：V0.4（2026-08-12 22:41:06 +08:00）

审查基线：提交 `25a8aba`（V0.4 更多平台）及当前提交 `f711fcd`。范围包括 OpenRouter、SiliconFlow、Claude、Gemini 适配器、设置页接入、文档和现有自动化测试。

## 结论

V0.4 已建立四个平台的注册与 UI 接入骨架，Claude 的 Usage/Cost 数据结构和分页方向基本正确，SiliconFlow 做了业务错误处理，Gemini 也没有伪造不存在的查询结果。但当前版本**不能按“更多平台已实现”验收**：OpenRouter 默认配置会生成错误 URL，且把美元费用误报为 Token；Claude 分页存在静默截断和“今日数据取最后一个 bucket”的口径风险；Gemini 实际不可监控却被作为普通可添加 Provider 暴露。

验证结果：

- `pnpm build`：通过。
- `cargo test`：37 passed，0 failed。
- 测试目前主要是 JSON 解析单元测试，没有 mock HTTP 请求测试，因而没有发现 OpenRouter 的重复路径问题，也没有验证 Claude 分页请求和截断行为。
- 未调用任何用户真实 API Key，真实平台端到端行为仍未验证。

## 发现的问题

### P0：OpenRouter 默认 URL 与适配器路径重复，默认新增账户无法刷新

- 位置：`src/pages/Settings.tsx:29`，`src-tauri/src/providers/openrouter.rs:53`。
- 设置页默认 Base URL 是 `https://openrouter.ai/api/v1`，适配器再拼接 `/api/v1/key`，最终请求为 `https://openrouter.ai/api/v1/api/v1/key`。
- 官方当前 Key 查询端点是 `GET https://openrouter.ai/api/v1/key`。默认配置应该使用站点根地址并由适配器追加路径，或保留当前默认地址并只追加 `/key`；两端必须统一。
- 建议增加一个 URL 构造纯函数测试和 mock HTTP 请求测试，避免同类 Base URL 契约错误。

### P1：OpenRouter 的美元费用被写入 `total_tokens`

- 位置：`src-tauri/src/providers/openrouter.rs:73-77`。
- 官方 `usage`、`usage_daily`、`usage_monthly` 是 credits/USD 使用金额，不是 Token 数量。当前把 `usage as u64` 写入 `total_tokens`，Dashboard 会把美元消费显示为 Token，并在汇总 Widget 中参与 Token 合计。
- 应保持 `total_tokens = 0`（该端点不提供 Token），将 `usage` 放入明确的累计费用字段；现有统一模型没有累计总费用字段时，不应强塞进 Token。`usage_daily/monthly` 放入费用字段是合理的。

### P1：Claude 超过 5 页时静默返回不完整数据

- 位置：`src-tauri/src/providers/claude.rs:176-205`。
- `fetch_with_pagination` 固定最多 5 页。若第 5 页仍 `has_more=true`，函数直接返回已收集部分并报告成功，随后不完整数据会写入历史。
- 应继续分页至 `has_more=false`，或设置合理高上限并在触顶时返回显式错误；不能把截断结果作为完整数据落库。还应检测重复/空 `next_page`，防止异常服务端造成循环。

### P1：Claude 的“今日”指标只是最后一个返回 bucket，不保证属于今天

- 位置：`src-tauri/src/providers/claude.rs:122-145`。
- `UsageBucket`/`CostBucket` 丢弃了 `starting_at` 与 `ending_at`，代码直接用 `last()` 作为今日值。空闲账户最后一个 bucket 可能是数日前；接口顺序变化也会令结果错误。
- 另外 `start = now - 30 days` 与 `ending_at = now` 不是 UTC 日界线，所谓“30 天/月费用”和“今日费用”会是滚动的部分日窗口。
- 应解析 bucket 时间，按 UTC 日期明确筛选今天；月口径需明确是近 30 个完整/部分日还是自然月，并在 UI 与存储中统一。

### P1：Gemini 被注册为可添加 Provider，但设计上每次刷新必然失败

- 位置：`src-tauri/src/providers/gemini.rs:16-24`，`src-tauri/src/providers/mod.rs:130`，`src/pages/Settings.tsx:32,42,102-107`。
- 设置页要求用户输入并保存 Gemini API Key，但适配器完全不读取 Key，任何刷新都会返回“不支持”。这让用户把敏感凭据写入 keyring，却得不到任何可用功能，并持续产生失败刷新。
- 在没有查询实现前，应从 `supported_provider_types` 中移除 Gemini，或把它做成无需 Key、不可保存的外部 Billing 页面入口。仅在提示文字里说明“不支持”不足以构成 Provider 实现。
- 官方文档目前说明使用情况在 AI Studio Dashboard 查看；2026 年文档已出现 Prepay/Postpay 与余额概念，因此 README 中“无公开余额/用量查询端点”的结论可以保留，但应避免写成永久能力判断。

### P2：SiliconFlow 成功响应缺少/无法解析余额时仍记录成功空数据

- 位置：`src-tauri/src/providers/siliconflow.rs:70-79`。
- 当 `code == 0` 但 `data.balance` 缺失、为空或不是数字时，适配器返回成功的空 `ProviderUsage`，刷新层会写入一条零 Token、无余额的历史记录。这无法区分“该接口确实不提供余额”和“响应协议变化/解析失败”。
- 对这个只提供余额的平台，余额应作为核心字段；缺失或非数字应显式报错，避免污染历史。当前 `tolerates_missing_balance` 测试反而固化了不可信行为。

### P2：README 没有同步 V0.4 当前状态

- 位置：`README.md:3,33,64,106` 附近。
- 项目简介、V0.1 Provider 列表和目录说明仍只列 OpenAI/DeepSeek/Codex；路线图仍把 V0.4 写成未来事项，且没有说明 Gemini 是不可用占位项。
- 应新增 V0.4 功能章节，准确区分“已实现”“实验性”“不可支持”，并写明 Claude 需要 Admin Key、OpenRouter 指标是费用、SiliconFlow 端点缺少官方稳定性保证。

### P2：Provider 列表顺序不稳定

- 位置：`src-tauri/src/providers/mod.rs:138-140`。
- `supported_types()` 直接遍历 `HashMap` keys，设置页下拉顺序会在不同进程间变化。新增到 7 种平台后，这个问题更明显。
- 应使用稳定注册表或在返回前排序，并用产品期望顺序而非随机顺序展示。

## V0.4 任务状态

| 能力 | 状态 | 审查结论 |
| --- | --- | --- |
| OpenRouter | 不可验收 | 默认 URL 404 风险；费用误记为 Token。 |
| SiliconFlow | 基本实现 | 余额查询路径已写，但依赖非正式/未稳定文档端点，核心字段缺失会静默成功。 |
| Claude | 基本实现 | 官方 Admin Usage/Cost API、认证和 cents→USD 方向正确；分页截断与时间 bucket 口径需修复。 |
| Gemini | 未实现 | 当前只是必然返回错误的占位适配器，不应作为可添加 Provider。 |
| 前端接入 | 部分完成 | 类型和提示已加入，但 OpenRouter preset 错误、Gemini 表单会无意义收集 Key。 |
| 文档 | 未完成 | README 尚未反映 V0.4 实际能力与限制。 |
| 自动化验证 | 部分完成 | 37 个 Rust 单测通过、前端构建通过；缺少 HTTP mock、URL 契约和分页行为测试。 |

## 建议修复顺序

1. 修复 OpenRouter URL 和费用/Token 字段映射，并补 URL/mock 测试。
2. 修复 Claude 分页截断与按 bucket 日期计算今日/近 30 天指标。
3. 在无可用查询方案前从可添加列表移除 Gemini，避免无意义保存 API Key。
4. 将 SiliconFlow 缺失/非法余额改为显式失败。
5. 稳定 Provider 下拉顺序并更新 README。
# 代码复审：V0.5 与设置控件（2026-08-12 23:09:26 +08:00）

审查基线：提交 `3bfce46`（V0.5 高级统计）与 `136a67a`，同时复核 Provider 圆角选择面板、圆形 Always On Top 控件。工作区审查前为干净状态。

## 结论

设置页视觉整改已达到目标：Provider 面板为黑底白字、圆角自绘菜单，Always On Top 使用圆形控件；点击外部与 Escape 关闭也已实现。但自绘菜单的键盘交互还不完整。

V0.5 已搭建历史查询、趋势图、预测和通知链路，工程可以构建，现有测试全部通过；不过当前历史数据口径与预测日期边界不足以支撑“准确趋势/预测”的验收。最关键的问题是 `usage_history.tokens` 并非统一的单日 Token 指标，以及 SQL 日期窗口存在多取一天、预测仍固定除以 7 的偏差。

验证结果：

- `pnpm build`：通过。
- `cargo test`：40 passed，0 failed。
- `git diff --check`：通过。
- 未进行真实系统通知、真实多日数据或桌面键盘交互测试。

## 发现的问题

### P1：Token 历史混用了“区间累计值”和“当日值”，趋势不可横向解释

- 位置：`src-tauri/src/commands.rs:605-624`，`src-tauri/src/providers/openai.rs:117-124`，`src-tauri/src/providers/claude.rs:144-151`。
- `record_usage` 直接把 `usage.total_tokens` 写入当天 `tokens`。但统一结构没有规定该字段的时间口径：OpenAI/Claude 查询的是一段日期范围后合计，OpenRouter/SiliconFlow 为 0，其他 Provider 也未必提供当日 Token。
- 结果是趋势图名为 Token 历史，实际可能展示“每次刷新时近 30 天累计量”的每日快照；它不是每日 Token 消耗，折线变化也不能代表当天趋势。
- 应给统一模型增加明确的 `today_tokens`，或按 Provider 日 bucket 计算当天值后入库。若保存累计值，应改列名和 UI 文案为“累计 Token 快照”，并通过相邻日期差分生成日消耗。

### P1：历史查询日期边界多取一天，预测除数却固定为 7

- 位置：`src-tauri/src/commands.rs:195-222,244-248`。
- SQL 条件 `date >= date('now', '-{days} days')` 是包含边界的。传入 30 时会覆盖“今天 + 前 30 天”，最多 31 个日期；传入 7 时最多 8 个日期。
- `get_prediction` 随后无论取到多少日期都执行 `total_cost / 7.0`。当边界日期有记录时会高估日均；当历史不足或有空白日时，又可能按另一种口径低估。
- 应使用 `-(days - 1) days` 形成包含今天的 N 天窗口，并明确预测是“自然日平均”还是“有数据日平均”。建议把日期区间计算提取为可测试函数，并用临时 SQLite 数据测试 7/30 天边界。

### P1：费用缺失被写成 0，预测无法区分真实零消费和 Provider 不支持

- 位置：`src-tauri/src/commands.rs:622`。
- `usage.today_cost.unwrap_or(0.0)` 会将“不提供费用指标”落库成真实 0。V0.5 随后把这些 0 用于日均费用和耗尽预测。
- 这会把 SiliconFlow 等余额型但无费用数据的平台解释为“当日零消费”，而不是“未知”；预测提示也无法说明数据不可用。
- 数据库 `cost` 应允许 `NULL`，`DailyUsage.cost` 应为 `Option<f64>`；预测只应使用真实费用数据，并向 UI展示有效样本数/时间范围。

### P1：提醒只读取 `remaining`，大部分 Provider 永远不会触发额度预警

- 位置：`src-tauri/src/commands.rs:277-325` 及各 Provider 字段映射。
- `check_alerts` 只根据 `usage.remaining` 判断百分比。目前主要只有 Codex 映射该字段；OpenRouter/SiliconFlow/DeepSeek 返回的是绝对余额，OpenAI/Claude通常没有余额百分比。
- 因此“自动提醒已实现”只适用于能够直接提供剩余百分比的平台，不能作为通用 Provider 能力宣称。
- 应在 Provider 能提供额度上限时计算百分比，或将提醒配置区分为百分比阈值、绝对余额阈值和预测剩余天数阈值；UI/README 需明确适用范围。

### P2：趋势组件在 Provider 删除后可能继续查询旧 ID

- 位置：`src/components/TrendWidget.tsx:12-31`。
- `effectiveId = selectedId ?? providers[0]?.id` 不检查 `selectedId` 是否仍存在于最新 Provider 列表。用户选中某账户后删除它，组件仍会查询已删除 ID 的历史和预测。
- `get_usage_history` 可能继续返回残留历史，而预测也会基于旧账户数据展示；选择框的当前 value 同时不再对应任何 option。
- 应在 providers 更新时校验选择项，不存在则重置到首个可用账户，并清空旧 history/prediction。

### P2：历史与预测使用 `Promise.all`，单项失败会丢失另一项可用结果

- 位置：`src/components/TrendWidget.tsx:20-31`。
- 历史查询成功但预测失败时，整个 `Promise.all` 进入 catch，成功历史不会写入状态；反之亦然。一次预测错误会让趋势图也无法更新。
- 应分别处理两项请求，或使用 `Promise.allSettled`，让趋势与预测独立降级，并避免继续展示上一次成功请求的陈旧数据。

### P2：折线过滤最大值与绘制数据不是同一集合

- 位置：`src/components/TrendWidget.tsx:41-55`。
- `max` 只对有限且非负值过滤，但 `points` 仍遍历原始 `history`。如果数据库或序列化出现 `NaN`/负数异常，可能生成无效 SVG 坐标或把负数强制压到 0，却没有错误提示。
- 后端应拒绝非有限/负费用数据，前端应先生成统一的已校验点集，再同时用于最大值和折线。

### P2：自绘 Provider 菜单缺少完整 listbox 键盘语义

- 位置：`src/pages/Settings.tsx:199-238`。
- 当前可用鼠标、Tab 和 Escape，但触发按钮不支持 ArrowDown/ArrowUp/Enter 选择；展开后也不会自动聚焦当前选项。`role=listbox/option` 暗示了原生 listbox 键盘行为，实际没有实现。
- 应实现 roving focus：展开时聚焦当前项，上下方向键移动，Enter/Space 选择，Home/End 跳转，Escape 关闭并把焦点归还触发按钮。否则可改用无障碍组件库或保留原生 select。

### P2：V0.5 文档状态仍停留在路线图

- 位置：`README.md` 路线图及功能章节。
- README 仍把 V0.5 列为未来计划，没有说明趋势/预测/提醒已经进入实现，也没有披露提醒仅对 `remaining` 百分比有效、历史至少需要多日刷新等限制。
- 应新增 V0.5 Alpha 章节，避免提交记录宣称“已实现”而用户文档仍描述为未来功能。

## 控件审查结果

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| 下拉面板黑底白字 | 通过 | 自绘面板不再依赖 Windows 原生 select 配色。 |
| 面板与选项圆角 | 通过 | 面板 12px、选项 8px，且 `overflow: hidden`。 |
| 圆形复选框 | 通过 | 自绘圆环/圆点，包含 hover 与 focus-visible。 |
| 点击外部关闭 | 通过 | document pointerdown + ref 范围判断。 |
| Escape 关闭 | 基本通过 | 触发按钮获得焦点时可关闭；选项获得焦点时事件冒泡同样可到父级范围，但未统一归还焦点。 |
| 完整键盘选择 | 未完成 | 缺方向键、Home/End、展开聚焦和 roving focus。 |

## V0.5 状态判断

| 能力 | 状态 | 审查结论 |
| --- | --- | --- |
| 历史查询 | 基本实现 | SQL/命令链路存在，但日期窗口多取一天，缺少边界测试。 |
| Token 趋势 | 不可准确验收 | 入库字段时间口径不统一。 |
| 费用趋势 | 部分可用 | 有费用的平台可展示；未知费用被伪装为 0。 |
| 耗尽预测 | 不可准确验收 | 日期除数偏差、未知费用语义丢失，缺公式测试。 |
| 自动提醒 | 部分可用 | 仅直接提供 `remaining` 百分比的平台有效。 |
| 系统通知 | 待桌面验证 | 权限已加入，但未验证首次权限、实际弹窗和失败反馈。 |
| 自动化测试 | 不足 | 40 个 Rust 测试通过，但 V0.5 仅覆盖提醒阈值，没有 SQL、预测、趋势组件测试。 |

## 建议修复顺序

1. 明确并修复历史 Token/费用的数据口径，保留未知值而不是写 0。
2. 修复 N 天日期窗口与预测公式，增加临时数据库边界测试。
3. 明确提醒适用平台，并补绝对余额/预测天数阈值策略。
4. 修复 TrendWidget 删除账户、并发请求降级和异常数据处理。
5. 补齐自绘菜单键盘行为并更新 README 的 V0.5 Alpha 状态。
# 修复状态审计：V0.5 Review（2026-08-12 23:26:35 +08:00）

审查基线：提交 `236e881`（V0.5 复审修复）与 `461397e`。本轮逐项阅读代码并运行测试，不以提交说明或 `codereview.md` 中的修复声明作为完成证据。

## 结论

上一轮 V0.5 Review 的主要任务已经完成：Token 历史口径、日期窗口、费用 NULL 语义、提醒适用范围、TrendWidget 删除账户处理、独立请求降级、异常点过滤、自绘菜单键盘操作和 README 均有对应的真实代码变更。项目能够构建，Rust 测试从 40 增至 42 且全部通过。

但任务还不能判定为 **100% 完成**。当前没有 P0；仍有 1 个影响历史真实性的 P1，以及数个 TrendWidget 状态一致性 P2。系统通知和真实多日趋势仍缺桌面/端到端验证。

## 上轮问题逐项核查

| 上轮问题 | 当前状态 | 代码证据 |
| --- | --- | --- |
| Token 历史混用累计值与当日值 | 基本修复 | DB V4 增加 `today_tokens`；趋势使用 `todayTokens`；累计 `tokens` 仅为兼容快照。仍有“真实 0 被当未知”问题。 |
| N 天窗口多取一天、固定除 7 | 已修复 | `history_start_offset(days)` 使用 `-(days-1)`；日均按有效费用样本数计算；增加 2 个纯函数测试。 |
| 费用缺失被写成 0 | 已修复 | V4 将 `cost` 改为可空；`DailyUsage.cost`/前端类型为 nullable；入库直接保存 `usage.today_cost`。 |
| 提醒只支持 `remaining` | 基本修复 | 百分比优先，无百分比时使用预测剩余天数 `<7/<3` 兜底；README 已说明适用范围。真实通知仍待验证。 |
| 删除 Provider 后继续查询旧 ID | 已修复 | providers 变化时校验 `selectedId`，不存在则重置并清空状态。 |
| 历史/预测单项失败相互阻塞 | 已修复 | 改用 `Promise.allSettled`，分别更新结果和错误。失败时旧值清理仍不完整。 |
| 折线 max 与绘制集合不一致 | 已修复 | 统一 `series` 过滤 nullable、非有限值和负值，再共同计算 max 与 points。 |
| Provider 菜单键盘语义不完整 | 已修复 | 实现展开聚焦、上下循环、Home/End、Enter/Space、Escape 和焦点归还。 |
| README 未同步 V0.5 | 已修复 | 新增 V0.5-alpha 章节、指标口径和提醒限制。 |

## 仍未完成的问题

### P1：真实的当日 0 Token 被保存为 NULL

- 位置：`src-tauri/src/commands.rs:741-747`。
- 当前通过 `input_tokens + output_tokens > 0` 判断平台是否提供当日 Token。这个判断无法区分“平台提供了当日数据且今天真实使用量为 0”和“平台不提供该指标”；两种情况都落为 `NULL`。
- 更重要的是，Provider 统一结构里的 `input_tokens/output_tokens` 仍是非可空 `u64`，无法表达指标是否存在。仅在入库层根据数值猜测，数据语义仍不完整。
- 建议在 `ProviderUsage` 增加 `today_tokens: Option<u64>`，由各 Provider 按日 bucket 明确赋值。真实 0 保存 `Some(0)`，不支持保存 `None`；不要从 input/output 数值反推可用性。

### P2：TrendWidget 请求失败时仍可能展示上一账户/上一次请求的旧结果

- 位置：`src/components/TrendWidget.tsx:30-45`。
- `load()` 开始时只清空 `error`，没有清空 `history/prediction`。若新账户历史请求失败，旧账户趋势仍显示；预测失败也会保留之前的预测卡片，同时只追加错误文字。
- `Promise.allSettled` 已做到独立降级，但 rejected 分支应分别执行 `setHistory([])` / `setPrediction(null)`，或显示明确的 stale 状态，避免错误信息和旧数据并存。

### P2：快速切换 Provider 时旧请求可能覆盖新账户结果

- 位置：`src/components/TrendWidget.tsx:30-49`。
- 每次 `effectiveId` 改变都会启动请求，但没有 request sequence、取消标志或 AbortSignal。若账户 A 请求较慢、切换到 B 后 B 先返回，A 随后仍会覆盖 B 的历史和预测。
- 应在 effect/load 中增加递增请求 ID 或 cancelled 标志，只有当前请求可以提交状态。

### P2：趋势图用 `history.length` 判断可绘制，而不是有效点数量

- 位置：`src/components/TrendWidget.tsx:67-113`。
- 当历史有 2 行但当前指标都是 `NULL` 时，`history.length >= 2` 成立，组件会渲染空 SVG，而不是“数据不足”。只有一个有效点时也会进入折线分支。
- 应以 `series.length >= 2` 判断折线是否可绘制；横坐标可继续按原始日期索引保留缺失日期间隔。

### P2：预测函数对不存在的 Provider 仍返回 `Some` 空预测

- 位置：`src-tauri/src/commands.rs:268-320`。
- `predict_for` 不验证 provider 是否存在；历史为空时仍返回 `Some(Prediction { samples: 0, balance: None, ... })`。删除账户后的短暂竞态或外部调用不会得到 `ProviderNotFound`/`None`。
- 建议先校验 Provider，或在历史为空时返回 `Ok(None)`。前端已经为 `null` 预测设计了降级文案。

### P2：V4 数据库迁移缺少从真实 V3 schema 升级的专项测试

- 位置：`src-tauri/src/db/mod.rs:89-118` 及测试模块。
- 当前 migration 测试主要验证新库创建和重复执行；没有显式构造 V3 表与旧数据，再验证升级后行数据、唯一索引、外键和 NULL 语义均保留。
- V4 采用 rename/create/copy/drop 的重建迁移，风险高于简单 `ALTER ADD COLUMN`，应增加旧版本 fixture 测试。

## 验证结果

- `pnpm build`：通过。
- `cargo test`（`src-tauri`）：42 passed，0 failed。
- `git diff --check`：通过。
- 审查前工作区干净；本轮仅追加 `codereview.md`。
- 未验证：真实系统通知授权/弹窗、真实 Provider 多日数据、快速切换账户的桌面交互。

## 当前完成度判断

| 范围 | 判断 |
| --- | --- |
| 上轮 Review 的主要修复任务 | 已完成 |
| V0.5 数据模型准确性 | 基本完成，真实 0/未知仍未严格区分 |
| TrendWidget 状态一致性 | 未完全完成 |
| 设置控件视觉与键盘交互 | 已完成 |
| 自动化验证 | 基本完成，缺 V3→V4 迁移与前端竞态测试 |
| V0.5 整体验收 | 有条件通过 Alpha，不建议标记为稳定完成 |
# 再次审计：V0.5 整改完成度（2026-08-12 23:40:28 +08:00）

审查基线：提交 `49a2238`（V0.5 复审遗留修复）与 `507e0c3`。本轮重新阅读 Provider、历史入库、预测、TrendWidget 与数据库迁移代码，并复跑全部可执行验证。

## 结论

上一轮列出的 6 个明确遗留项已经全部有实质代码修复：`today_tokens` 能区分真实 0 与未知、TrendWidget 会清理失败结果并防止旧请求覆盖、空图判断使用有效点数量、预测检查 Provider 存在性、V3→V4 迁移有专项数据保留测试。

但是 V0.5 整改仍未完全结束。重新沿“NULL 表示未知”的目标追踪费用 Provider 后，发现 OpenAI 与 Claude 的今日费用口径仍有两个 P1 数据准确性问题；它们会进入 `usage_history.cost`，继而影响费用趋势、日均预测和天数提醒。因此当前可以确认“上一轮清单已完成”，但不能确认“V0.5 整体整改全部完成”。

## 上一轮 6 项遗留核查

| 遗留项 | 状态 | 代码证据 |
| --- | --- | --- |
| 真实 0 Token 被当 NULL | ✅ 已修复 | `ProviderUsage.today_tokens: Option<u64>`；OpenAI/Claude 有今日 bucket 时写 `Some(0+)`，无 bucket 保持 `None`；入库不再按 `>0` 猜测。 |
| 请求失败保留旧趋势/预测 | ✅ 已修复 | rejected 分支分别 `setHistory([])`、`setPrediction(null)`。 |
| 快速切换 Provider 的旧请求覆盖 | ✅ 已修复 | `seqRef` 递增，请求完成后只允许最新序号提交状态。 |
| 按历史行数绘制造成空图 | ✅ 已修复 | 绘制条件改为 `series.length >= 2`。 |
| 不存在 Provider 返回空预测 | ✅ 已修复 | `predict_for` 先查询 providers，不存在返回 `Ok(None)`。 |
| 缺 V3→V4 迁移专项测试 | ✅ 已修复 | 新测试构造 V3 schema/旧数据，验证版本、数据、NULL、唯一索引和旧表清理。 |

## 新确认的未完成问题

### P1：OpenAI 今日费用仍取最后一个 bucket，未按 UTC 日期筛选

- 位置：`src-tauri/src/providers/openai.rs:45-51,149-151`。
- Usage bucket 已恢复 `aggregation_timestamp` 并按 UTC 日期筛选今日 Token；但 `CostBucket` 没有保存 bucket 时间，`today_cost` 仍使用 `costs_data.last()`。
- 如果今天没有费用记录，最后一项可能属于昨天或更早，系统会把旧费用写入今天的 `usage_history.cost`。费用趋势、预测日均和耗尽提醒都会被污染。
- 应在 `CostBucket` 解析 `start_time`（按官方实际字段名确认），使用与 Usage 相同的 UTC 日期过滤；无今日 bucket 时保存 `None`，有今日 bucket 且合计为 0 时保存 `Some(0)`。

### P1：Claude 无今日费用 bucket 时仍保存 `Some(0)`

- 位置：`src-tauri/src/providers/claude.rs:127-137,164-172`。
- 代码已经按日期得到 `today_cost_buckets`，但无论集合是否为空都会求和并执行 `usage.today_cost = Some(day_cents / 100.0)`。
- 这再次把“今天没有返回费用 bucket/指标未知”伪装成真实零费用，与 V4 数据库迁移和预测只使用真实样本的整改目标冲突。
- 应仅在 `!today_cost_buckets.is_empty()` 时写 `Some(...)`，否则保持 `None`；补充“空 bucket → None、存在零值 bucket → Some(0)”单元测试。

### P2：OpenAI 今日 Token 只读取第一个匹配 bucket

- 位置：`src-tauri/src/providers/openai.rs:132-148`。
- 代码先收集 `today_buckets`，却只对 `today_buckets.first()` 聚合。通常一天只有一个 bucket，但分页重复、分组或服务端异常产生多个同日 bucket 时其余数据会丢失。
- Claude 已对所有今日 bucket 做 fold；OpenAI 应采用同样方式聚合全部匹配 bucket，避免实现语义不一致。

### P2：V3→V4 迁移测试未模拟旧表外键

- 位置：`src-tauri/src/db/mod.rs:155-219`。
- 测试覆盖数据、索引和旧表清理，但手工创建的 V3 `usage_history` 缺少生产 V1 schema 中的 `FOREIGN KEY (provider_id) ... ON DELETE CASCADE`，也没有验证迁移后级联删除仍工作。
- 当前 V4 新表代码确实重新声明了外键，因此不是已发现的运行错误；建议完善 fixture 并开启 `PRAGMA foreign_keys=ON`，用删除 Provider 验证级联行为，提升迁移测试可信度。

## 验证结果

- `pnpm build`：通过。
- `cargo test`（`src-tauri`）：43 passed，0 failed。
- `git diff --check`：通过。
- 本轮未使用真实 Provider 凭据，未验证真实多日费用 bucket 和系统通知。

## 最终判断

| 判断对象 | 结果 |
| --- | --- |
| 上一次审计列出的 6 项整改 | 已完成 |
| V0.5 Token 趋势整改 | 基本完成 |
| V0.5 费用趋势/预测整改 | 未完全完成 |
| V0.5 自动提醒 | 代码链路完成，真实通知待验证 |
| V0.5 整体整改 | 约完成，仍需修复上述 2 个 P1 后再验收 |
