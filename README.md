# AI API Monitor

## 项目简介

AI API Monitor 是一个基于 Tauri 2、React、TypeScript、Rust 和 SQLite 的跨平台桌面应用，用于集中查看多个 AI Provider 的余额、Token、费用、趋势、预测和提醒，并提供 Full/Mini/Ball 悬浮模式、主题与布局能力。

## 当前功能

- Provider 管理与刷新：Claude、Codex、DeepSeek、OpenAI、OpenRouter、SiliconFlow；Gemini 当前未注册。
- 余额、Token、费用、历史趋势、消耗预测和低额度提醒。
- Full/Mini/Ball 窗口模式、托盘集成、主题切换、Widget 显隐/排序。
- 系统凭据库保存 API Key，SQLite 保存凭据引用；图片/GIF 资源隔离与安全校验。
- 统一 UI 控件、背景/主题资源和基础 Miuix/液态玻璃视觉结构。

## 当前项目结构

```text
src/                 React 页面、组件、状态、主题与测试
src-tauri/src/       Rust 命令、Provider、SQLite、窗口、安全与资源模块
public/              字体与主题资源
Markdown/            项目文档、任务、测试、安全报告与阶段摘要
.github/workflows/   CI 质量门禁
package.json         前端与 Rust 检查脚本
```

## Markdown 文档索引

- [文档总索引](Markdown/Project/项目索引.md)
- [项目维护说明](Markdown/Project/README_Project.md)
- [开发指南](Markdown/Development/DEVELOPMENT_GUIDE.md)
- [TodoList](Markdown/TODO_LIST.md)
- [Codex 阶段摘要](Markdown/Summaries/Codex_Project_History_Summary_2026-08-14.md)
- [项目状态摘要](Markdown/Summaries/README_Project_Status.md)
- [主测试清单](Markdown/Tests/TEST_CASES.md)
- [扩展测试资料](Markdown/Tests/AIAPIMonitor_TestCases.md)
- [安全审计报告](Markdown/Security/Security_Audit_Report.md)
- [安全测试报告](Markdown/Security/Security_Test_Report.md)
- [安全操作手册](Markdown/Security/Manual_Security_Operations.md)
- [代码复审记录](Markdown/CodeReview/codereview.md)
- [优化任务](Markdown/CodeReview/优化建议.md)
- [变更日志](Markdown/Development/CHANGELOG.md)
- [Windows 启动说明](Markdown/Development/README_Startup.md)

## 已完成功能

以当前代码和自动化测试可确认的范围为准：系统凭据库与 Key 脱敏、资源安全校验、基础 Provider 管理、主题与布局基础能力、趋势/预测/提醒、窗口状态同步和统一选择控件已实现。详细边界见 [TodoList](Markdown/TODO_LIST.md) 与 [安全审计](Markdown/Security/Security_Audit_Report.md)。

## 当前未完成任务

Provider HTTP Mock 与真实接口验证、桌面 E2E、多平台凭据库/窗口验证、旧凭据迁移、诊断备份恢复、完整 DIY UI、生产自动更新配置仍未完成。不要仅凭历史审查中的“已修复”文字勾选任务。

## 最近主要修改

- 安全模块覆盖凭据、网络、日志、敏感设置、图片/GIF 与平台目录。
- 增加主题资源、背景裁剪、Miuix 风格组件和统一控件。
- 整理项目文档目录，迁移根目录项目 Markdown，统一 `Markdown/Tests` 命名并修复相对路径。

## 安全状态

安全基线已进入代码和自动化测试，但 RustSec/依赖审计、真实平台凭据库、网络边界、资源手工攻击样本和发布签名仍需执行。生产自动更新当前只能视为安全禁用的集成骨架。

## 测试状态

TodoList 记录的历史审计为前端 15 项、Rust 91 项通过；本次在当前环境重新运行时，pnpm 依赖重建因并发安装出现 `ENOENT`，随后 TypeScript 因不完整 `node_modules` 失败，因此本次不能把质量门禁标记为新近通过。详见 [TodoList](Markdown/TODO_LIST.md)。

## 后续开发重点

1. 修复并稳定依赖安装后重新执行 `pnpm check`、`pnpm build` 与 `pnpm security:audit`。
2. 建立 Provider HTTP Mock 合约测试，补齐真实权限、限流和错误场景。
3. 完成真实 Tauri 桌面冒烟、托盘/悬浮窗/多屏和跨平台凭据库验证。
4. 完成 updater 发布配置、诊断备份恢复和剩余高优先级安全手工验收。

## 文档整理记录

最后整理时间：2026-08-14 15:10 (UTC+8)

本次扫描项目自有 Markdown、可见 Codex 历史摘要和当前代码；根目录项目文档已迁移至 `Markdown/Development` 或 `Markdown/Summaries`，测试目录统一为 `Markdown/Tests`，旧安全原稿归档至 `Markdown/Archive`，并更新索引与相对路径。依赖、构建产物、`.git`、`.reasonix`、`node_modules` 和工具/技能自身 Markdown 未移动。
