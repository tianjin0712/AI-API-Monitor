import { invoke } from "@tauri-apps/api/core";
import type {
  DailyUsage,
  DeleteResult,
  Prediction,
  ProviderConfig,
  ProviderUsage,
  RefreshResult,
  RefreshSettings,
  AppBehaviorSettings,
  UpdateInfo,
  WindowMode,
  WindowState,
  ImportedAsset,
} from "./types";

/** 前端 -> Rust 后端 invoke 封装 */
export const api = {
  importAsset: (originalName: string, bytes: Uint8Array) =>
    invoke<ImportedAsset>("import_asset", {
      originalName,
      data: Array.from(bytes),
    }),
  deleteAsset: (assetId: string) => invoke<void>("delete_asset", { assetId }),
  readAsset: (assetId: string) => invoke<number[]>("read_asset", { assetId }),

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

  deleteProvider: (id: number) => invoke<DeleteResult>("delete_provider", { id }),

  supportedProviderTypes: () => invoke<string[]>("supported_provider_types"),
  isCustomEndpointApproved: (apiUrl: string) =>
    invoke<boolean>("is_custom_endpoint_approved", { apiUrl }),
  approveCustomEndpoint: (apiUrl: string) =>
    invoke<string>("approve_custom_endpoint", { apiUrl }),

  refreshProvider: (id: number) =>
    invoke<ProviderUsage>("refresh_provider", { id }),

  refreshAll: () => invoke<RefreshResult[]>("refresh_all"),
  getCodexRuntimeStatus: () => invoke<{ installed: boolean; loggedIn: boolean; runtimeSource: string | null }>("get_codex_runtime_status"),
  startCodexLogin: () => invoke<void>("start_codex_login"),

  getRefreshSettings: () => invoke<RefreshSettings>("get_refresh_settings"),

  setRefreshSettings: (foregroundSecs: number, backgroundSecs: number) =>
    invoke<void>("set_refresh_settings", { foregroundSecs, backgroundSecs }),
  getAppBehaviorSettings: () => invoke<AppBehaviorSettings>("get_app_behavior_settings"),
  setCloseBehavior: (closeBehavior: string) => invoke<void>("set_close_behavior", { closeBehavior }),
  setAutoStart: (enabled: boolean) => invoke<boolean>("set_auto_start", { enabled }),

  // ---- V0.2 窗口能力 ----
  setWindowMode: (mode: WindowMode) =>
    invoke<WindowState>("set_window_mode", { mode }),
  setAlwaysOnTop: (enabled: boolean) =>
    invoke<WindowState>("set_always_on_top", { enabled }),
  snapWindowToWorkArea: () => invoke<void>("snap_window_to_work_area"),
  getWindowState: () => invoke<WindowState>("get_window_state"),
  getMigrationStatus: () => invoke<number | null>("get_migration_status"),
  getDatabaseRecoveryNotice: () => invoke<string | null>("get_database_recovery_notice"),

  // ---- V0.3 DIY UI ----
  getLayout: () => invoke<string | null>("get_layout"),
  setLayout: (layout: string) => invoke<void>("set_layout", { layout }),

  // ---- V0.5 高级统计 ----
  getUsageHistory: (providerId?: number | null, days?: number) =>
    invoke<DailyUsage[]>("get_usage_history", { providerId, days }),
  getPrediction: (providerId: number) =>
    invoke<Prediction | null>("get_prediction", { providerId }),

  // ---- V1.0 自动更新 ----
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  installUpdate: (expectedVersion: string) => invoke<string>("install_update", { expectedVersion }),
};
