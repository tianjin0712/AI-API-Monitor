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
  | "gemini";

export interface ProviderConfig {
  id: number;
  name: string;
  providerType: string;
  apiUrl: string;
  enabled: boolean;
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
  todayCost: number | null;
  monthCost: number | null;
  remaining: number | null;
  resetTime: string | null;
  updatedAt: string;
}

export interface RefreshSettings {
  foregroundSecs: number;
  backgroundSecs: number;
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
export type WidgetType = "providers" | "summary" | "cost";

/** Dashboard 上的一个 Widget */
export interface DashboardWidget {
  id: string;
  type: WidgetType;
  visible: boolean;
}

/** DIY 布局（JSON 持久化，含主题） */
export interface Layout {
  theme: "dark" | "light";
  widgets: DashboardWidget[];
}
