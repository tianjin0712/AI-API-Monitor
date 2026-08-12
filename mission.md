下面是整理给 **Reasonix / AI 编程代理** 使用的项目方案文档。定位是让 Reasonix 能根据该 Markdown 直接进行项目初始化、架构设计和后续开发。

```
:::writing{variant="document" id="58391" title="AI API Token Monitor 桌面软件开发方案"}
# AI API Token Monitor 开发方案

## 1. 项目名称

AI API Monitor

目标：
开发一个跨平台桌面软件，用于实时监控多个 AI API 平台的 Token 使用情况、余额、额度、费用，并提供高度可定制化 UI、悬浮窗、小球模式。

主要支持：

- OpenAI API
- Codex Usage
- DeepSeek API
- OpenRouter
- Anthropic Claude
- Google Gemini
- SiliconFlow
- 自定义 OpenAI Compatible API


---

# 2. 核心目标

创建一个类似：

- GPU Monitor
- Rainmeter
- macOS Widget
- Windows 小组件

的 AI 资源监控工具。

核心能力：

1. 实时查看 API 剩余额度
2. 查看 Token 消耗情况
3. 查看每日/月度费用
4. 多 API 平台统一管理
5. 桌面悬浮显示
6. 小球模式
7. UI 自定义布局
8. 主题系统
9. 额度不足提醒


---

# 3. 技术方案


## 推荐技术栈

采用：
```

Tauri 2
+
React
+
TypeScript
+
Rust
+
SQLite

```
原因：

- 跨 Windows / macOS
- 软件体积小
- 内存占用低
- 支持透明窗口
- 支持置顶窗口
- 支持系统托盘
- 支持悬浮窗
- 支持原生安全存储


---

# 4. 软件整体架构
```

AI API Monitor

│
├── Frontend
│
│   React + TypeScript
│
│   ├── Dashboard
│   ├── Floating Window
│   ├── Mini Ball
│   ├── Theme Editor
│   └── Settings
│

├── Backend

│   Rust Core
│
│   ├── API Provider Manager
│   ├── Token Tracker
│   ├── Scheduler
│   ├── SQLite Database
│   ├── Secure Storage
│   └── Window Controller

└── Providers

```
├── OpenAI
├── Codex
├── DeepSeek
├── OpenRouter
├── Anthropic
├── Gemini
└── Custom Provider
---

# 5. Provider 设计


每个平台独立 Adapter。


目录：
```

src/providers/

├── openai.ts
├── codex.ts
├── deepseek.ts
├── openrouter.ts
├── anthropic.ts
├── gemini.ts
└── custom.ts

```
统一数据格式：

​```typescript
interface ProviderUsage {

    provider:string;

    balance:number;

    currency:string;


    totalTokens:number;

    inputTokens:number;

    outputTokens:number;

    cachedTokens:number;


    todayCost:number;

    monthCost:number;


    remaining:number;

    resetTime:string;


    updatedAt:string;

}
```

新增平台时：

只需要增加 Provider。

无需修改 UI。

------

# 6. 支持平台

## DeepSeek

支持：

- 查询余额
- 查询 Token 使用
- 查询费用

显示：

```
DeepSeek

余额

¥48.32


今日消耗

120K Tokens


预计剩余

15天
```

------

## OpenAI API

支持：

- Usage API
- Cost API
- Token统计

显示：

```
OpenAI

Today

Input:
1.2M


Output:
320K


Cost:

$2.31
```

------

## Codex

注意：

Codex ChatGPT额度

与 OpenAI API Key 不同。

需要单独处理：

```
Codex Provider

读取：

- Usage Dashboard
- CLI Status
- 本地状态
```

不要与 OpenAI API 混合。

------

# 7. 数据库设计

SQLite。

## Provider 表

```
providers

id

name

type

api_url

key_id

created_time
```

API Key 不保存明文。

使用：

Windows:

Credential Manager

macOS:

Keychain

------

## Usage 表

```
usage_history


id

provider

date

tokens

cost

balance
```

用于生成：

- 日报
- 周报
- 月报
- 消耗曲线

------

# 8. UI设计

## 主界面

Dashboard:

```
--------------------------------

AI API Monitor


OpenAI

████████░░

72%


DeepSeek

█████████

¥48.32


Anthropic

██████░░

60%



Today Cost

¥12.32


--------------------------------
```

------

# 9. DIY UI 系统

支持 Widget 化。

组件：

```
Widget

├── BalanceWidget

├── TokenWidget

├── CostWidget

├── ProgressWidget

├── ChartWidget

├── ProviderWidget

└── ResetTimerWidget
```

用户可以：

- 拖动
- 缩放
- 删除
- 隐藏
- 调整透明度
- 调整圆角
- 修改字体
- 修改颜色

布局保存：

JSON。

例如：

```
{
 "theme":"dark",

 "opacity":0.9,

 "widgets":[

    "openai",

    "deepseek",

    "cost"

 ]

}
```

------

# 10. 悬浮窗系统

支持三种状态。

## 完整模式

```
+----------------+

 OpenAI 72%

 DeepSeek ¥42

 Cost ¥12

+----------------+
```

## Mini模式

```
+-------------+

AI 72%

+-------------+
```

## 小球模式

```
      ○

     72%
```

功能：

- 拖动
- 吸附屏幕边缘
- 鼠标穿透
- 自动隐藏
- 点击展开

------

# 11. 提醒系统

额度：

> 50%

正常

<30%

黄色提醒

<10%

红色提醒

支持：

- Windows Notification
- macOS Notification

------

# 12. 后台刷新策略

不要高频请求。

策略：

打开软件：

10秒刷新

后台：

60秒刷新

点击刷新：

立即刷新

电脑唤醒：

立即刷新

------

# 13. 预计消耗预测

根据历史数据计算。

例如：

```
DeepSeek


剩余:

¥42


最近7天:

¥5/day


预计:

8.4天


预计耗尽:

2026-08-20
```

------

# 14. 自定义 Provider

必须支持。

用户填写：

```
Name

Base URL

API Key

Balance API

Usage API
```

用于支持：

- 国内代理
- 私有部署
- 公司API

------

# 15. 开发阶段

## V0.1 基础版本

完成：

- Tauri项目初始化
- React UI
- SQLite
- DeepSeek余额
- OpenAI Usage
- Provider管理

目标：

可以显示数据。

------

## V0.2 桌面能力

增加：

- 系统托盘
- Always On Top
- Mini窗口
- 小球模式

------

## V0.3 UI编辑器

增加：

- Widget拖拽
- Layout保存
- 主题系统

------

## V0.4 更多平台

增加：

- Codex
- Claude
- Gemini
- OpenRouter

------

## V0.5 高级统计

增加：

- Token历史
- 消耗趋势
- 费用预测
- 自动提醒

------

## V1.0

正式版：

- 插件化Provider
- 主题分享
- 自动更新
- Windows/macOS发布

------

# 16. 开发要求

代码要求：

- 模块化
- Provider独立
- 前后端分离
- 所有配置可扩展
- 禁止硬编码

UI要求：

- 支持深色模式
- 支持透明窗口
- 支持毛玻璃效果
- 支持高DPI

安全要求：

- API Key必须加密保存
- 禁止明文保存Key

------

# 17. 第一阶段执行任务

Reasonix 开始开发时：

1. 创建 Tauri 2 项目
2. 配置：

```
React
TypeScript
Rust
SQLite
```

1. 完成窗口系统：
   - 主窗口
   - 无边框窗口
   - 透明窗口
   - Always On Top
2. 创建 Provider 架构
3. 首先实现：
   - DeepSeek Provider
   - OpenAI Provider
4. 完成 Dashboard
5. 再扩展：
   - 悬浮球
   - DIY UI
   - 更多平台

------

最终目标：

打造一个类似 Rainmeter 的 AI API 资源监控中心。

用户无需打开网页，即可在桌面实时查看所有 AI 服务：

- Token
- 余额
- 花费
- 使用趋势
- 剩余时间

并通过自定义 UI 形成个人 AI 工作台。
:::
\```
