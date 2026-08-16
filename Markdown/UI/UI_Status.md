# UI 系统状态

## 已实现

- `src/pages/Dashboard.tsx` 与 `src/pages/Settings.tsx` 提供主界面和设置页。
- `src/components/ProviderCard.tsx`、`TrendWidget.tsx`、`TitleBar.tsx`、`MiniBall.tsx` 组成核心桌面 UI。
- `src/components/ui/Controls.tsx`、`AppSelect.tsx` 统一输入框、下拉框、Button、Switch、Checkbox 和 Tooltip 交互。
- `src/styles/miuix.css` 与 `miuix-official.css` 提供 Miuix/液态玻璃 token、卡片、焦点态和背景混合效果。
- Widget 布局支持排序、显隐、添加/删除，并持久化到后端布局 JSON。

## 设计方向

继续沿用 Miuix 的低噪音层级、圆角卡片、明确焦点态和一致控件尺寸；保留用户自定义主题、壁纸和颜色覆盖，不把视觉风格硬编码为单一主题。

## 待验证

- Windows/macOS 真机上的 DPI、拖动、悬浮窗点击与 Tooltip 边界。
- 完整 DIY（自由定位、缩放、透明度、字体/颜色）尚未全部实现。
- 视觉回归截图基线尚未建立。
