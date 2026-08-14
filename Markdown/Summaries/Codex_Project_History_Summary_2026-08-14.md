# Codex 项目对话阶段摘要

整理日期：2026-08-14

## 已提出并形成代码或文档的方向

- 项目定位为 Tauri 2 + React + TypeScript + Rust + SQLite 的跨平台 AI API 监控桌面应用。
- 目标能力包括 Provider 管理、余额/Token/费用/趋势/预测、Full/Mini/Ball 悬浮模式、主题与布局定制。
- 已形成 DeepSeek、OpenAI、Claude、Codex、OpenRouter、SiliconFlow 的 Provider 适配路径；Gemini 保留实现但未注册。
- 已形成 Miuix/液态玻璃风格、壁纸背景、主题色提取和自定义主题方向。

## 已完成或当前代码可确认

- API Key 通过系统凭据库保存，SQLite 只保存引用；UI 有脱敏路径。
- 图片/GIF 导入有资源隔离、格式/尺寸/帧数限制和活动 SVG 拒绝逻辑。
- 网络请求、安全日志脱敏、敏感设置加密、平台安全目录和安全测试模块已进入代码。
- Provider CRUD、主题基础能力、Widget 显隐/排序、趋势预测、提醒和统一 AppSelect 键盘交互已存在。
- Full/Mini/Ball 的核心事件路径存在；但真实桌面事件、托盘、拖动、双击、Hover 和多屏行为仍需手测。

## 历史 Bug 修复与 UI 修改

- 对托盘模式切换与 React 状态同步进行了修复，使用 `window-mode-changed` 事件同步。
- 统一了 UI 控件、主题变量、液态玻璃样式、背景裁剪和主题资源结构。
- 最近一次对话提出重新梳理悬浮窗状态机、单击/双击/拖动冲突及 250ms 内部 Hover 提示；可见摘要只确认需求已转交 Work 会话，不能据此判定实现完成。

## 未完成与后续重点

- Provider HTTP Mock 合约测试、真实权限/限流/错误场景和桌面 E2E。
- 旧凭据迁移、凭据删除失败恢复、诊断导出、数据库备份/恢复。
- 完整 DIY UI（自由定位、缩放、透明度、字体、颜色、添加/删除 Widget）。
- 自动更新生产公钥、HTTPS endpoint、签名产物和升级/篡改/降级验收。
- macOS/Windows/Linux 真机凭据库、通知、窗口、安装包和发布验证。

## 依据与限制

本摘要来自当前项目 Markdown、当前代码扫描、可见 Codex 压缩摘要和相关历史任务标题；未把历史对话中的规划或“已完成”表述直接当作当前代码事实。
