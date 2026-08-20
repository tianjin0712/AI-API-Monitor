# 当前 TODO

> 当前口径：最近发布基线 Tag 为 `v1.0.6`；`origin/master` 另有 4 个未打 Tag 的自定义 API Provider 提交，当前开发主线版本待下一次发布确定。产品成熟度为 V1.0-alpha。代码实现、自动化测试和真实桌面/发布验收必须分别记录；只有后者完成后才能标记为发布完成。

## P0 发布与数据安全

1. 合并当前未打 Tag 的主线提交后，统一新 Git Tag、三个受控 manifest 和安装包版本；确认版本同步结果可复现。
2. 恢复并追踪 P0 数据恢复验收清单，完成损坏库、锁冲突、快照、WAL、旧凭据迁移、删除补偿和 Canary 泄漏扫描。
3. 配置自动更新生产公钥、HTTPS endpoint 和签名产物，并验收合法包、篡改包、错误签名包与降级包。
4. 清理发布基线：工作区无无关改动、`git diff --check` 通过、构建缓存不进入版本库。

## P1 当前版本完善

1. 完成 OpenAI、OpenRouter、Claude、SiliconFlow 和 Codex 的真实账户权限、限流、分页、错误和 CLI 版本兼容测试。
2. 完成 macOS/Linux 的 Keychain/Secret Service、通知、窗口、DPI、多屏、安装和升级验证；Windows 针对恢复与升级场景复回归。
3. 建立桌面 E2E 和视觉回归，覆盖托盘、Full/Mini/Ball、拖动、Tooltip、设置、主题和布局恢复。
4. 统一 README、TODO、架构、安全报告和验收清单的版本与完成口径。
5. 实现脱敏诊断导出及面向用户的数据库备份/恢复入口。

## P2 架构与质量

1. 设计版本化 Layout schema，补齐自由定位、缩放与保存失败恢复的基础模型。
2. 统一 Provider 元数据，并建立前后端 DTO 契约或快照测试。
3. 拆分大型 Rust/React 模块；固化时区、通知去重/冷却和安全审计策略。
4. 规范提交说明、Tag 描述和自动生成的发布说明。

## P3 产品演进

1. 完整 DIY UI、主题分享和跨设备配置同步。
2. 真正的动态 Provider 插件加载及 Gemini 等新 Provider。
3. 日报/周报/月报和成本预测增强。

历史完成项、实现证据和历史质量门禁记录保留在 [`TODO_LIST.md`](./TODO_LIST.md) 与 [`../Review/codereview.md`](../Review/codereview.md)。
