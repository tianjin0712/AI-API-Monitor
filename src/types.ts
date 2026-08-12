// 与 Rust 后端交互的类型定义（字段 camelCase 与 ProviderUsage/ProviderConfig 对齐）

export type ProviderType = "deepseek" | "openai" | "codex" | "custom";

export interface ProviderConfig {
  id: number;
  name: string;
  providerType: string;
  apiUrl: string;
  keyRef: string;
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
