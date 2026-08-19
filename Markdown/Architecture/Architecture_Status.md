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

自动更新生产配置、旧数据迁移、真实桌面 E2E、跨平台凭据库和多屏行为仍是发布前工作，不应在文档中标记为已完成。
