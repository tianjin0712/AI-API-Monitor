# 架构状态

## 分层

```text
React/Vite UI
  └─ src/api.ts / MonitorStore / refreshLogic
      └─ Tauri commands.rs
          ├─ ProviderManager + providers/*
          ├─ settings.rs / storage.rs
          ├─ db/mod.rs (SQLite)
          └─ window_mode.rs / assets.rs / security.rs
```

## 已确认的关键设计

- Provider 通过 `ProviderAdapter` trait 插件化，新增适配器后由 `ProviderManager::new()` 注册。
- `custom` 为通用自定义 API 适配器（`providers/custom.rs`）：非敏感配置 JSON 存 `providers.custom_config`（schema V7），敏感值经 keyring；通过 JSON 点路径映射 + 单位（token/count/currency/custom）映射到统一 `ProviderUsage`（`usage.custom` 承载原始 remaining/total/used/unit）。
- 前台/后台刷新由状态层单飞调度，后端再次校验间隔；历史数据进入 SQLite，趋势和预测在前端计算。
- 窗口状态由 Rust 管理，React 监听 `window-mode-changed`；Full/Mini/Ball 对应产品文档的 MAIN/FLOAT_SQUARE/FLOAT_EXPANDED。
- 主题与 Widget 布局作为一份受校验的 JSON 保存，资源导入走 Tauri 资源协议和后端隔离目录。

## 当前边界

- 自动更新生产配置、签名产物和升级验收仍是发布前工作。
- 旧凭据 UUID 迁移、删除补偿、数据库恢复和迁移快照已有代码路径；真实凭据库、故障注入和安装版恢复验收仍未完成，不能按发布完成标记。
- Windows 日常窗口、多屏和 DPI 已有实机记录；真实桌面 E2E、macOS/Linux 凭据库与窗口行为仍需验证。
