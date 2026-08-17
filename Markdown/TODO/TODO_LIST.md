# AI API Monitor Todo List

> 整理日期：2026-08-14  
> 当前版本：V0.5-alpha / V1.0 alpha 能力已进入代码库  
> 审计日期：2026-08-14  
> 平台优先级：**优先在 Windows 台式机（i5-13600KF + RTX 4070）上运行与优化，验证通过后再复刻到 macOS 端**。CI（`.github/workflows/quality.yml`）即跑在 `windows-latest`，与主平台一致；macOS 本地跑通是次一级的交叉验证。  
> 说明：本清单以当前代码、测试结果、Git 历史和 Markdown 文档为准；仅有文档或历史对话声明的任务不会直接勾选。
> 完成时间规范：每个已勾选条目备注完成时间（YYYY-MM-DD HH:MM，以 git 提交时间/验证记录时间为准），后续每完成一项即同步勾选并补时间。

## 一、发布前必须完成

- [x] 在当前目标目录重新安装依赖：`pnpm install --frozen-lockfile`。2026-08-14 19:22 验证通过（含删除 node_modules 后的全新重建，lockfile 与声明一致）。
- [x] 执行 `pnpm check`，确认 TypeScript、Vitest、Rust fmt、Clippy 和 Rust 测试全部通过。2026-08-14 19:59 复现通过：前端 58 项、Rust 136 项全绿（93 基线 + 43 HTTP Mock + 39 前端 + 4 Widget 新增；此前记录的“依赖重建 ENOENT、类型库路径缺失”已定位根因，见第十四节）。
- [x] 执行 `pnpm build`，确认生产构建成功。2026-08-14 19:22 验证通过（tsc + vite build，56 模块，dist 产物正常）。
- [x] 安装并执行 `cargo-audit` / `pnpm security:audit`。2026-08-14 19:22 验证通过：JS 依赖 0 漏洞；Rust 扫描 591 个依赖 0 漏洞、17 条允许的传递依赖 warning（非漏洞，不阻塞）。
- [ ] 完成 Windows 真实 Tauri 桌面环境冒烟测试。
- [ ] 完成 Provider 真实接口和权限行为验证。
- [ ] 配置自动更新签名公钥、HTTPS 更新地址和签名产物。
- [ ] 验证自动更新的合法包、篡改包、错误签名包和降级版本。

## 二、桌面窗口与托盘

- [x] 修复托盘切换 Full/Mini/Ball 后前端 React 状态不同步的问题。代码已通过 `window-mode-changed` 事件同步。
- [ ] 解决托盘左键菜单与左键显示/隐藏行为冲突，确定唯一交互方案。状态：需要真实桌面运行验证。
- [ ] 手测窗口关闭到托盘、托盘显示/隐藏、模式切换和真正退出。
- [ ] 完善 Full、Mini、小球三种模式的窗口位置和尺寸持久化。
- [x] 支持多显示器位置恢复，并在副屏断开后进行边界校正。2026-08-17 已在真实多显示器和不同 DPI 环境验证移动、显示及副屏断开后的边界恢复正常。
- [ ] 处理窗口状态或数据库保存失败时的回滚和状态一致性。
- [ ] 实现电脑唤醒后的自动刷新。
- [ ] 评估并实现窗口吸附、鼠标穿透和自动隐藏。
- [ ] 手测 Always On Top，并验证重启后状态恢复。

## 三、数据迁移与安全存储

- [ ] 增加旧版本 `provider_<name>` 凭据引用到 UUID 引用的数据库迁移。状态：代码已补强为幂等迁移；数据库失败会清理新引用并保留旧记录，待 Rust/Keychain 集成测试。
- [ ] 验证迁移失败时不会静默丢失凭据，并提示用户重新录入。状态：失败记录保留并写入脱敏日志/迁移状态；待故障注入测试。
- [ ] 为凭据删除失败增加用户可见提示、待清理记录和启动重试机制。状态：代码已改为凭据删除失败时保留 Provider，数据库删除失败时补偿恢复凭据；待真实凭据库故障测试。
- [ ] 在 Windows、macOS、Linux 真实环境验证系统凭据库行为。
- [ ] 验证应用数据目录、数据库、日志和崩溃信息中不出现完整 API Key、Token 或 Cookie。

## 四、Provider 与网络测试

- [x] 建立 Provider HTTP Mock 合约测试基础设施。2026-08-14 19:49 落地：`src-tauri/src/providers/test_http.rs`（本地脚本化 Mock 服务器：状态码/头/体/延迟/断连/路径绑定/请求记录）+ `http_contract_tests.rs`（43 项合约测试）；生产安全客户端不变，仅新增 `#[cfg(test)]` 测试客户端与 `fetch_usage_with_client` 注入点。
- [x] 为各 Provider 覆盖成功响应、401/403、429、5xx、超时和网络中断。2026-08-14 19:49：DeepSeek/OpenRouter/SiliconFlow/Claude/OpenAI 五个 HTTP Provider 已按该矩阵全覆盖（Codex 为 CLI 通道，无 HTTP 端点）；真实账户权限行为仍需真机验证。
- [x] 覆盖非 JSON、字段缺失、分页、NaN、负数和协议变化。2026-08-14 19:49：非 JSON 与字段缺失由 HTTP Mock 覆盖（含 SiliconFlow 余额缺失显式报错、OpenRouter 可选字段缺失）；分页由 Claude/OpenAI 双页聚合与重复 cursor 测试覆盖；NaN/负费用由 `validate_cost`/`parse_cents` 纯函数测试覆盖。协议变化（字段缺失/格式非法）已覆盖。
- [ ] 验证 OpenAI Organization Usage/Costs 的管理员权限和分页行为。
- [ ] 验证 Claude 管理员接口和费用/余额缺失时的展示逻辑。
- [ ] 验证 Codex CLI 未登录、登录过期和接口变化时的错误提示。
- [x] 验证 SiliconFlow 余额字段缺失时不会写入虚假零值。代码和 Rust 测试已确认显式报错；真实接口仍需验证。
- [ ] 确认 Gemini 是否继续保持未注册状态，或补齐正式实现后再开放。
- [ ] 统一 Provider 元数据，由后端返回类型、默认 URL、能力和凭据模式。状态：当前仍有前端 `TYPE_PRESETS` 等静态元数据。

## 五、前端功能与自动化测试

- [x] 补充 Dashboard 刷新、单飞控制和刷新竞态测试。2026-08-14 19:57：从 `MonitorStore` 提取 `src/state/refreshLogic.ts` 纯函数并新增 17 项测试（前台/后台间隔钳制、结果合并、单飞/失败判定、刷新状态机）；组件级渲染交互仍需桌面 E2E。
- [ ] 补充 TrendWidget 趋势刷新、预测降级和无效数据测试。状态：预测降级/无效数据分支已补（`prediction.test.ts` 现 6 项，覆盖 daysLeft 缺失与 null 输入）；TrendWidget 组件级趋势刷新仍需 E2E。
- [ ] 补充 Settings 表单校验、凭据编辑和保存失败重试测试。状态：未开始。
- [x] 补充刷新间隔 fake timer 测试。2026-08-14 19:57：`computeRefreshIntervalSecs`（前台最小 10s、后台最小 60s、可见性切换）已覆盖；调度 Effect 本身需组件测试环境。
- [ ] 补充布局解析、布局防抖、拖拽排序和恢复默认测试。状态：部分完成；布局解析已有测试，交互和失败重试仍缺。
- [ ] 补充托盘事件同步和桌面 E2E 测试。
- [ ] 完善主题切换和主题持久化验证。状态：主题资源 id/路径映射与自定义资源安全回退2026-08-14 19:57 已测（`themeAssets.test.ts` 10 项，含非安全 URL fail-closed）；主题切换/持久化交互仍需 E2E。
- [x] 完善统一 Dropdown 的方向键、Home/End、Escape 和 ARIA 基础支持。`AppSelect` 已被 Provider、设置和趋势控件复用；仍需运行时无障碍验证。

## 六、DIY UI 与布局系统

- [ ] 设计版本化 Layout schema，支持向前兼容。
- [x] 支持 Widget 删除、添加和恢复默认。2026-08-14 19:59：编辑模式新增「删除」按钮与「添加 Widget」下拉（`AppSelect` + 四类可选，每类最多一个实例，id 由 `nextWidgetId` 保证不冲突）；恢复默认沿用既有按钮；拖拽排序与显示/隐藏维持原有行为。布局经 `onWidgetsChange` 统一持久化。新增 `nextWidgetId` 纯函数测试 4 项（Vitest 58/58）。
- [ ] 支持 Widget 自由定位和尺寸调整。
- [ ] 支持透明度、圆角、字体大小、字体和颜色编辑。
- [ ] 支持完整的 Widget 布局保存、加载和失败恢复。
- [ ] 在产品文档和界面中明确当前 DIY 功能边界。

## 七、诊断、备份与恢复

- [ ] 增加脱敏诊断日志导出。
- [ ] 增加数据库备份功能。状态：迁移前自动快照已实现；面向用户的手动备份入口仍未实现。
- [ ] 增加数据库恢复功能，并要求用户明确确认。状态：损坏数据库会保留原文件并以安全空库启动，恢复入口仍未实现。
- [ ] 在数据库损坏、目录不可写或锁冲突时提供可恢复启动界面。状态：损坏/迁移失败会保留原库并弹出恢复提示；锁冲突会重试并给出明确启动错误，待桌面故障注入测试。
- [ ] 在数据库迁移前自动创建可恢复快照。状态：已实现版本+时间戳+随机标识命名，保留最近 5 份；待 Rust 测试验证。

## 八、跨平台与正式发布

- [ ] 在 macOS 真机验证 Keychain、通知权限和窗口行为。
- [ ] 在 Linux 真实会话验证 Secret Service 和文件权限。
- [ ] 在 macOS 执行 DMG 构建、代码签名和安装测试。
- [ ] 验证 Windows/macOS 安装包升级流程。
- [ ] 验证不同企业代理环境下的 HTTPS、重定向和证书行为。
- [ ] 验证真实生产 API 的限流策略、费用统计口径和时区口径。
- [ ] 确认发布矩阵、应用图标、版本号和品牌资源。

## 九、长期优化

- [ ] 拆分大型 Rust 和 React 模块，降低页面和命令层耦合。
- [ ] 建立前后端 DTO 自动生成或契约快照测试。
- [ ] 明确 UTC 日历日与用户本地时区的产品规则。
- [ ] 持久化通知去重状态，并设计冷却和恢复阈值。
- [ ] 评估证书固定及其证书轮换策略。
- [ ] 规划插件化 Provider 和主题分享能力。

## 十、手动测试清单

发布前优先执行以下测试用例：

- [ ] TC-003、TC-004、TC-007
- [ ] TC-017
- [ ] TC-019～TC-026
- [ ] TC-029
- [ ] TC-034～TC-045
- [ ] TC-050

安全手工验证：

- [ ] 导入 20MB 边界图片、4096×4096 图片和 301 帧 GIF。
- [ ] 导入伪装扩展名文件和活动 SVG，确认被拒绝。
- [ ] 使用无效证书、HTTP endpoint、跨域 30x 重定向和系统代理测试 fail-closed。
- [ ] 检查 SQLite、日志、崩溃报告和系统日志，不包含真实敏感信息。
- [ ] 在三平台分别新增、编辑、删除 Provider，确认凭据创建、更新和删除正常。

## 完成标准

- [ ] 所有 P0/P1 任务完成或有明确延期记录。
- [ ] `pnpm check`、`pnpm build` 和安全审计通过。
- [ ] 关键手动测试全部记录结果和问题。
- [ ] Windows、macOS、Linux 的平台特有功能完成验证。
- [ ] 正式发布配置、签名产物和升级流程验证完成。

## 十一、已确认实现但仍需运行验证

### Windows 实机记录（2026-08-17）

- [x] 托盘、Full/Mini/Ball 切换、关闭到托盘、开机自启、置顶和拖拽已完成实测。
- [x] 多显示器与不同 DPI 缩放组合已完成实测，窗口显示、交互及副屏断开后的边界恢复正常。
- [x] Codex Runtime 子进程不再弹出空白控制台：`login status`、登录、后台额度监听和手动额度读取均使用 Windows 无控制台创建标志。
- [x] Codex 本地运行时识别正常；DeepSeek 使用实际 API 连接正常。
- [ ] OpenAI、OpenRouter、Claude、SiliconFlow 尚未配置可供验证的 API/管理员凭据，当前未测试；不应据此标记为失败或通过。

- [x] 悬浮球单击切换为 Mini，Mini 单击切回 Ball，Mini 双击打开 Full：`src/components/MiniBall.tsx` 已有对应事件路径。
- [x] 悬浮窗 Hover 约 1 秒显示 Tooltip：`MiniBall.tsx` 使用 1000ms timer。
- [x] 悬浮窗额度自动滚动和鼠标滚轮切换：设置值已持久化到 Layout，代码已接入 `MiniBall`。
- [x] Provider 管理和六类已注册 Provider：注册表及设置页路径存在，真实账户行为仍需手测。
- [x] API Key 使用系统凭据库、UI 脱敏和 Codex 不读取认证文件：自动安全测试已通过。
- [x] 图片/GIF 资源路径隔离、格式/尺寸/帧数限制和活动 SVG 拒绝：自动安全测试已通过。

以上项目只能确认“代码路径和自动化测试存在”；不能替代真实 Tauri 窗口、Hover、拖动、滚轮、凭据库和 Provider 网络测试。

## 十二、部分完成或被范围收缩的任务

- [ ] 完整 DIY UI。状态：部分完成；当前支持主题、主题色、Widget 显隐和排序，缩放、自由定位、删除、透明度、圆角、字体和颜色编辑仍未实现。
- [ ] V1.0 插件化 Provider。状态：部分完成；`custom` 当前是 OpenAI Admin 协议别名，不是真正的动态插件系统。
- [ ] 自动更新。状态：部分完成；安全守卫、版本升级/降级判断和设置 UI 已完成，生产公钥、HTTPS endpoint 和签名 E2E 尚未完成。
- [ ] 测试用例。状态：部分完成；50 条测试文档已生成，但未证明全部实际执行，且需要补充最新功能、HTTP Mock 和桌面 E2E。
- [ ] 主题分享。状态：部分完成；主题保存/加载代码存在，但重启持久化和完整校验契约仍需验证。

## 十三、审计结论

- 历史任务总数：已结合当前会话、可见会话摘要、项目文档、Git 历史和当前代码归并；由于摘要是压缩后的上下文，无法保证逐条恢复每一条历史原始消息，因此仍以代码证据和最新需求为准。
- 已完成或代码/自动测试确认：质量门禁、前端统一 AppSelect、悬浮窗核心事件路径、安全基线、SiliconFlow 缺失余额显式失败、若干 Provider 解析/分页/迁移逻辑。
- 部分完成：Provider HTTP 测试、完整测试矩阵、DIY UI、自动更新、插件化 Provider、主题分享。
- 仍未完成：旧凭据迁移、凭据删除失败恢复、系统唤醒刷新、诊断备份恢复、真实跨平台发布验证、生产 updater 配置。
- 需要实际运行验证：托盘行为、窗口模式切换、双击/Tooltip/拖动/滚轮、设置恢复、多显示器、真实 Provider、OS 凭据库和安装升级。
- 当前最高优先级：
  1. 完成真实 Tauri 桌面冒烟和托盘/悬浮窗行为验证。
  2. 建立 Provider HTTP Mock 合约测试并执行真实权限/错误场景验证。
  3. 完成 updater 签名配置、跨平台凭据库测试和发布前安全手工检查。

## 十四、质量门禁验证记录（2026-08-14 19:22）

验证日期：2026-08-14。环境：macOS（arm64）、Node v26.7.0、pnpm 11.21.0、Rust 1.97.1（clippy + rustfmt）。

> 平台说明：主目标平台为 Windows 台式机（i5-13600KF + RTX 4070），CI 即运行于 `windows-latest`，因此本记录的 macOS 本地验证属于交叉验证；Windows 端最终以 CI 结果为准，桌面冒烟等真实运行验证见第一节待办。`desktop_runtime.rs` 的测试修复为平台自适应写法（Windows 上仍匹配 `codex.exe`），对 Windows CI 无回归。

执行命令（严格按序）：

```bash
pnpm install --frozen-lockfile   # 通过（含全新重建；--store-dir 指向 /tmp，因家目录不可写）
pnpm check                        # 通过：tsc --noEmit ✓、Vitest 15/15 ✓、cargo fmt --check ✓、clippy -D warnings ✓、cargo test 93/93 ✓
pnpm build                        # 通过：tsc && vite build，56 模块，dist 产物正常
pnpm security:audit               # 通过：pnpm audit --prod 0 漏洞；cargo audit 扫描 591 依赖，0 漏洞、17 条允许的传递依赖 warning（gtk-rs/unic-*/proc-macro-error 等 unmaintained/unsound，非漏洞，不阻塞）
```

最终结果：四条门禁全部真实通过，可复现（同一内容在 APFS 卷副本上冷启动单次连续全过）。

本轮修复与根因：

1. 依赖安装问题的根因（历史“依赖重建 ENOENT / 类型库路径缺失”同源）：
   - 本机缺 pnpm 与 Rust 工具链（PATH 中无 pnpm/cargo/rustc），已安装 pnpm 11.21.0 与 Rust 1.97.1。
   - `src-tauri/target` 残留 33GB 的 Windows x86_64 旧构建产物（含 ai-api-monitor.exe/PDB/NSIS），与 macOS arm64 构建目标冲突，已删除（该目录被 gitignore）。
   - 工作区位于 Tuxera NTFS 外置卷（`synchronous` 挂载），Tuxera 驱动不稳定：cargo 构建出现 ENOENT/空 fingerprint/rlib 缺失（E0463），vite 构建时原生二进制 mmap 触发 SIGBUS 或在 `open()` 系统调用永久挂起。**验证改在 APFS 卷副本上执行后，全部步骤稳定通过**；在原 NTFS 位置原地执行仍可能复现挂起，需修复/更换 Tuxera 驱动或改用 APFS 卷（非代码问题，代码基线本身通过）。
   - `pnpm install` 在无 TTY 下需 `CI=true` 才能清理旧 modules 目录。
2. 代码修复：`src-tauri/src/providers/desktop_runtime.rs` 的 5 个测试写死了 Windows 文件名 `codex.exe`，而解析器按平台 `cfg` 只匹配 Windows 的 `codex.exe/.cmd`、其他平台的 `codex`，导致 macOS/Linux 本地必挂（unwrap None）。已改为平台自适应文件名（`runtime_name()`），测试意图不变；Windows CI 行为不受影响。

残余项：

- `cargo audit` 17 条 warning（全部为传递依赖“unmaintained/unsound”，无漏洞），与 CI 一致（`cargo audit --file` 默认不因 warning 失败）。
- 工作区所在 NTFS 卷的 Tuxera 驱动不稳定是当前唯一环境级阻塞（建议迁移到 APFS 卷或修复驱动后原地复跑确认）。

## 十四之二、质量门禁复跑记录（2026-08-14 19:32，基线提交 081940b）

在提交 `081940b`（质量门禁基线）之后对当前 HEAD 完整复跑，结果与首次验证一致：

- 执行命令（严格按序）与最终结果：
  1. `pnpm install --frozen-lockfile` —— 通过（仓库原位执行，2.9s；lockfile 与声明一致）
  2. `pnpm check` —— 通过（tsc ✓、Vitest 15/15 ✓、cargo fmt ✓、clippy -D warnings ✓、cargo test 93/93 ✓）
  3. `pnpm build` —— 通过（tsc && vite build，56 模块，dist 正常）
  4. `pnpm security:audit` —— 通过（JS 0 漏洞；cargo audit 591 依赖 0 漏洞、17 条允许的传递依赖 warning）
- 复跑环境变化：机器重启导致 `/tmp` 工具链（pnpm/Rust/cargo-audit）被清空，已按 rsproxy 镜像重建（rustup 1.29.0 + Rust 1.97.1 + clippy/rustfmt + cargo-audit 0.22.2）；仓库内混入从 Windows 同步回来的 `node_modules`（win32 原生二进制）与 5.3GB `src-tauri/target`（x86_64-pc-windows-msvc 产物）已清理（均为 gitignore 目录，不影响版本库）。
- 验证方式与残余项不变：完整链路在内容与仓库完全一致的 APFS 卷副本上冷启动单次连续通过（`diff -rq` 确认无差异）；NTFS/Tuxera 驱动不稳定仍是唯一环境级阻塞（非代码问题），Windows 端已在台式机实跑通过，与 CI（windows-latest）口径一致。

## 十四之三、质量门禁验证记录（2026-08-14 19:49，Provider HTTP Mock 合约测试落地）

在 HEAD 新增 Provider HTTP Mock 合约测试（`providers/test_http.rs` + `providers/http_contract_tests.rs`，43 项）后，完整四门禁复跑通过：

- `pnpm install --frozen-lockfile` —— 通过（lockfile 与声明一致）
- `pnpm check` —— 通过（tsc ✓、Vitest 15/15 ✓、cargo fmt --check ✓、clippy -D warnings ✓、cargo test **136/136** ✓，两次连续运行稳定）
- `pnpm build` —— 通过（tsc && vite build，56 模块，dist 正常）
- `pnpm security:audit` —— 通过（JS 0 漏洞；cargo audit 591 依赖 0 漏洞、17 条允许的传递依赖 warning）
- 验证方式同上：APFS 卷副本（内容与仓库一致，`diff -rq` 无源码差异）冷启动单次连续全过；生产安全策略未被放宽（测试客户端仅 `#[cfg(test)]`，`fetch_usage_with_client` 为纯注入点，生产路径仍走 `secure_http_client`）。

## 十四之四、质量门禁验证记录（2026-08-14 19:57，前端自动化测试补充）

新增前端测试（`refreshLogic.test.ts` 17 项、`format.test.ts` 10 项、`themeAssets.test.ts` 10 项、`prediction.test.ts` 扩至 6 项；`MonitorStore` 提取 `refreshLogic.ts` 纯函数，行为不变）后，完整四门禁复跑通过：

- `pnpm install --frozen-lockfile` —— 通过（lockfile 与声明一致）
- `pnpm check` —— 通过（tsc ✓、Vitest **54/54**（6 个文件）✓、cargo fmt --check ✓、clippy -D warnings ✓、cargo test 136/136 ✓）
- `pnpm build` —— 通过（tsc && vite build，56 模块，dist 正常）
- `pnpm security:audit` —— 通过（JS 0 漏洞；cargo audit 591 依赖 0 漏洞、17 条允许的传递依赖 warning）
- 验证方式同上：APFS 卷副本（内容与仓库一致）冷启动单次连续全过。

## 十五、Windows 启动器修复记录

- [x] 修复 `Start_AI_API_Monitor.bat`（2026-08-15 14:48）：移除首行 UTF-8 BOM（`cmd.exe` 将 BOM 视为首行命令前缀，导致 `@echo off` 失效、全部命令被回显）；`:deps_missing` 由“提示后退出”改为自动执行 `call pnpm install --frozen-lockfile` 安装依赖（Windows 侧副本该分支曾因缺 `echo` 把提示当命令执行而报 `'录中执行：pnpm' is not recognized`），安装失败有明确错误指引；保持 `chcp 65001` 与 `call pnpm` 调用方式，行尾统一 CRLF、UTF-8 无 BOM。本改动为启动器脚本，不在 tsc/vitest/cargo 门禁路径内，四门禁不受影响。
