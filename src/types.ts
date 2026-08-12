// 与 Rust 后端交互的类型定义（字段 camelCase 与 ProviderUsage/ProviderConfig 对齐）

export type ProviderType = "deepseek" | "openai" | "codex" | "custom";

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
