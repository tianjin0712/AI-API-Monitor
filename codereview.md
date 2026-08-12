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
