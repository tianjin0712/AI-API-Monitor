import { invoke } from "@tauri-apps/api/core";
import type {
  ProviderConfig,
  ProviderUsage,
  RefreshSettings,
} from "./types";

/** 前端 -> Rust 后端 invoke 封装 */
export const api = {
  listProviders: () => invoke<ProviderConfig[]>("list_providers"),

  addProvider: (input: {
    name: string;
    providerType: string;
    apiUrl: string;
    apiKey: string;
  }) => invoke<ProviderConfig>("add_provider", input),

  updateProvider: (input: {
    id: number;
    name: string;
    apiUrl: string;
    apiKey?: string | null;
  }) => invoke<ProviderConfig>("update_provider", input),

  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),

  supportedProviderTypes: () => invoke<string[]>("supported_provider_types"),

  refreshProvider: (id: number) =>
    invoke<ProviderUsage>("refresh_provider", { id }),

  refreshAll: () => invoke<ProviderUsage[]>("refresh_all"),

  getRefreshSettings: () => invoke<RefreshSettings>("get_refresh_settings"),

  setRefreshSettings: (foregroundSecs: number, backgroundSecs: number) =>
    invoke<void>("set_refresh_settings", { foregroundSecs, backgroundSecs }),
};
