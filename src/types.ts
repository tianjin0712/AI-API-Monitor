// 与 Rust 后端交互的类型定义（字段 camelCase 与 ProviderUsage/ProviderConfig 对齐）

/**
 * 已注册的 Provider 类型（与后端 ProviderManager 注册表一致）。
 * 下拉选项来自 supported_provider_types 命令（动态），此处仅为静态提示。
 */
export type ProviderType =
  | "deepseek"
  | "openai"
  | "codex"
  | "openrouter"
  | "siliconflow"
  | "claude"
  | "custom";

export interface ProviderConfig {
  id: number;
  name: string;
  providerType: string;
  apiUrl: string;
  keyHint: string;
  enabled: boolean;
  /** 通用自定义 API 的非敏感配置（JSON 字符串，仅 custom 类型）。 */
  customConfig?: string | null;
  createdTime: string;
  updatedTime: string;
}

export interface ProviderUsage {
  providerId: number | null;
  provider: string;
  balance: number | null;
  currency: string;
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  /** 当日 Token（null=平台不提供/未知；0 为真实零消耗） */
  todayTokens: number | null;
  todayCost: number | null;
  monthCost: number | null;
  remaining: number | null;
  resetTime: string | null;
  /** Codex Desktop/App Server 返回的动态额度详情；其他 Provider 不提供。 */
  codex?: CodexUsageDetails | null;
  /** 自定义 API 的原始额度结果；其他 Provider 不提供。 */
  custom?: CustomUsageDetails | null;
  updatedAt: string;
}

export interface CodexUsageDetails {
  runtimeSource: string;
  planType: string | null;
  credits: unknown | null;
  windows: CodexRateLimitWindow[];
}

export interface CodexRateLimitWindow {
  limitId: string | null;
  limitName: string | null;
  windowKind: string;
  usedPercent: number;
  remainingPercent: number;
  windowDurationMins: number | null;
  /** Unix 时间戳（秒）。 */
  resetsAt: number | null;
  unlimited: boolean;
  tokenLimit?: number;
  tokensUsed?: number;
  tokensRemaining?: number;
}

export interface RefreshSettings {
  foregroundSecs: number;
  backgroundSecs: number;
}
export interface AppBehaviorSettings {
  closeBehavior: "minimize_to_tray" | "quit";
  autoStart: boolean;
}

/** refresh_all 的逐账户刷新结果 */
export interface RefreshResult {
  providerId: number;
  provider: string;
  success: boolean;
  usage: ProviderUsage | null;
  error: string | null;
}

/** 窗口模式（对应后端 WindowMode） */
export type WindowMode = "full" | "mini" | "ball";

/** 窗口状态 */
export interface WindowState {
  mode: WindowMode;
  alwaysOnTop: boolean;
}

/** 删除 Provider 的结果（凭据清理状态） */
export interface DeleteResult {
  providerId: number;
  credentialCleaned: boolean;
  note: string | null;
}

// ---- V0.3 DIY UI ----

/** Widget 类型（Dashboard 区块） */
export type WidgetType = "providers" | "summary" | "cost" | "trend";

/** Dashboard 上的一个 Widget */
export interface DashboardWidget {
  id: string;
  type: WidgetType;
  visible: boolean;
}

/** DIY 布局（JSON 持久化，含主题） */
export interface Layout {
  theme: "dark" | "light";
  /** 可选视觉主题；洛天依主题会应用专属配色与动态角色。 */
  visualTheme?: "default" | "luotianyi" | "custom";
  /** 主页面与悬浮窗口使用的洛天依 GIF，可独立于视觉主题选择。 */
  avatarGif?: string;
  /** 洛天依主题背景（内置背景 ID 或用户自定义背景 ID）。 */
  luotianyiBackground?: string;
  /** 液态玻璃卡片不透明度（0.15–0.9）。 */
  glassOpacity?: number;
  /** 壁纸模式下的背景模糊强度（0–32px）。 */
  glassBlur?: number;
  /** Mini 悬浮窗专用文字色；未设置时跟随主题主文字。 */
  miniTextColor?: string;
  /** 悬浮窗额度模块切换方式。 */
  floatingScrollMode?: "auto" | "wheel";
  widgets: DashboardWidget[];
  /** V1.0 自定义主题：CSS 变量名（不含 --color- 前缀）→ 色值 */
  themeOverrides?: Record<string, string>;
}

// ---- V0.5 高级统计 ----

/** 单日用量（历史序列数据点） */
export interface DailyUsage {
  date: string;
  /** 累计 Token 快照（兼容历史，非当日趋势指标） */
  tokens: number;
  /** 当日 Token（null=平台不提供/未知） */
  todayTokens: number | null;
  /** 当日费用（null=平台不提供/未知，不伪装成 0） */
  cost: number | null;
  balance: number | null;
}

/** 消耗预测 */
export interface Prediction {
  dailyCostAvg: number;
  /** 参与平均的有效费用样本数 */
  samples: number;
  /** 覆盖天数跨度 */
  daysSpan: number;
  balance: number | null;
  daysLeft: number | null;
  exhaustedDate: string | null;
}

/** V1.0 更新检查结果 */
export interface UpdateInfo {
  available: boolean;
  version: string | null;
  notes: string | null;
}

/** Opaque application-owned image resource; never contains a user file path. */
export interface ImportedAsset {
  assetId: string;
  url: string;
}

// ---- 通用自定义 API ----

export type CustomAuthType = "bearer" | "apiKey" | "basic" | "none" | "customHeader";
export type CustomUnit = "token" | "count" | "currency" | "custom";

export interface CustomKeyValue {
  key: string;
  value: string;
}

export interface CustomAuth {
  type: CustomAuthType;
  headerName?: string | null;
  username?: string | null;
}

export interface CustomResponseMapping {
  remainingPath?: string | null;
  totalPath?: string | null;
  usedPath?: string | null;
  resetTimePath?: string | null;
}

export interface CustomApiConfig {
  url: string;
  method: string;
  query: CustomKeyValue[];
  headers: CustomKeyValue[];
  body?: string | null;
  auth: CustomAuth;
  responseMapping: CustomResponseMapping;
  unit?: CustomUnit | null;
}

export interface CustomUsageDetails {
  remaining: number | null;
  total: number | null;
  used: number | null;
  unit: string;
}

export interface CustomTestResult {
  success: boolean;
  status: number | null;
  remaining: number | null;
  total: number | null;
  used: number | null;
  unit: string;
  resetTime: string | null;
  responsePreview: string | null;
  error: string | null;
}
