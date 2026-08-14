import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import type { ProviderConfig, ProviderUsage, RefreshSettings } from "../types";
import {
  computeRefreshIntervalSecs,
  mergeErrorResults,
  mergeUsageResults,
  nextRefreshState,
  refreshSucceeded,
} from "./refreshLogic";

export type RefreshState = "idle" | "refreshing" | "success" | "error";

interface MonitorStoreValue {
  providers: ProviderConfig[];
  usages: Record<number, ProviderUsage>;
  errors: Record<number, string>;
  refreshingIds: Set<number>;
  manualRefreshingIds: Set<number>;
  refreshState: RefreshState;
  lastUpdated: number | null;
  historyRevision: number;
  refreshAll: () => Promise<void>;
  refreshOne: (id: number) => Promise<void>;
  reloadProviders: () => Promise<void>;
}

const MonitorStoreContext = createContext<MonitorStoreValue | null>(null);

export function MonitorStoreProvider({ children }: { children: ReactNode }) {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [usages, setUsages] = useState<Record<number, ProviderUsage>>({});
  const [errors, setErrors] = useState<Record<number, string>>({});
  const [refreshingIds, setRefreshingIds] = useState<Set<number>>(new Set());
  const [refreshState, setRefreshState] = useState<RefreshState>("idle");
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  const [historyRevision, setHistoryRevision] = useState(0);
  const [refreshSettings, setRefreshSettings] = useState<RefreshSettings>({ foregroundSecs: 10, backgroundSecs: 60 });
  const providersRef = useRef(providers);
  const refreshingRef = useRef(false);
  const initializedRef = useRef(false);
  const hasCacheRef = useRef(false);
  const refreshAllRef = useRef<(showIndicator?: boolean) => Promise<void>>(async () => {});
  const [manualRefreshingIds, setManualRefreshingIds] = useState<Set<number>>(new Set());

  useEffect(() => { providersRef.current = providers; }, [providers]);
  useEffect(() => { hasCacheRef.current = Object.keys(usages).length > 0; }, [usages]);

  const reloadProviders = useCallback(async () => {
    const list = await api.listProviders();
    setProviders(list);
  }, []);

  const refreshAll = useCallback(async (showIndicator = true) => {
    if (refreshingRef.current || providersRef.current.length === 0) return;
    refreshingRef.current = true;
    setRefreshingIds(new Set(providersRef.current.map((provider) => provider.id)));
    if (showIndicator) setManualRefreshingIds(new Set(providersRef.current.map((provider) => provider.id)));
    setRefreshState("refreshing");
    try {
      const list = await api.refreshAll();
      setUsages((current) => mergeUsageResults(current, list));
      setErrors((current) => mergeErrorResults(current, list));
      setLastUpdated(Date.now());
      const succeeded = refreshSucceeded(list);
      if (succeeded) setHistoryRevision((value) => value + 1);
      setRefreshState(nextRefreshState(list));
    } catch (error) {
      setRefreshState("error");
      setErrors((current) => ({ ...current, _global: String(error) }));
    } finally {
      setRefreshingIds(new Set());
      if (showIndicator) setManualRefreshingIds(new Set());
      refreshingRef.current = false;
    }
  }, []);

  const refreshOne = useCallback(async (id: number) => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    setRefreshingIds(new Set([id]));
    setManualRefreshingIds(new Set([id]));
    setRefreshState("refreshing");
    try {
      const usage = await api.refreshProvider(id);
      setUsages((current) => ({ ...current, [id]: usage }));
      setErrors((current) => { const next = { ...current }; delete next[id]; return next; });
      setLastUpdated(Date.now());
      setHistoryRevision((value) => value + 1);
      setRefreshState("success");
    } catch (error) {
      setErrors((current) => ({ ...current, [id]: String(error) }));
      setRefreshState("error");
    } finally {
      setRefreshingIds(new Set());
      setManualRefreshingIds(new Set());
      refreshingRef.current = false;
    }
  }, []);

  useEffect(() => { refreshAllRef.current = refreshAll; }, [refreshAll]);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([reloadProviders(), api.getRefreshSettings()]).then(([, settings]) => {
      if (!cancelled) setRefreshSettings(settings);
    }).catch(() => {});
    return () => { cancelled = true; };
  }, [reloadProviders]);

  useEffect(() => {
    if (!initializedRef.current && providers.length > 0 && Object.keys(usages).length === 0) {
      initializedRef.current = true;
      void refreshAll(false);
    }
  }, [providers.length, usages, refreshAll]);

  useEffect(() => {
    const unlisten = listen<ProviderUsage>("codex-rate-limits-updated", (event) => {
      setUsages((current) => {
        const next = { ...current };
        for (const provider of providersRef.current.filter((item) => item.providerType === "codex")) {
          next[provider.id] = { ...event.payload, providerId: provider.id };
        }
        return next;
      });
      setLastUpdated(Date.now());
      setRefreshState("success");
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);

  useEffect(() => {
    const onProvidersChanged = () => {
      initializedRef.current = false;
      setUsages({});
      setErrors({});
      void reloadProviders();
    };
    window.addEventListener("providers-changed", onProvidersChanged);
    return () => window.removeEventListener("providers-changed", onProvidersChanged);
  }, [reloadProviders]);

  useEffect(() => {
    const intervalOf = () =>
      computeRefreshIntervalSecs(
        refreshSettings,
        document.visibilityState === "visible",
      ) * 1000;
    let timer: number | undefined;
    const tick = () => { void refreshAllRef.current(false); timer = window.setTimeout(tick, intervalOf()); };
    timer = window.setTimeout(tick, intervalOf());
    const onFocus = () => {
      // Focus is not a refresh trigger by itself; only refresh when no cache
      // exists or the normal scheduler has elapsed.
      if (!hasCacheRef.current) void refreshAllRef.current(false);
    };
    window.addEventListener("focus", onFocus);
    return () => { if (timer !== undefined) window.clearTimeout(timer); window.removeEventListener("focus", onFocus); };
  }, [refreshSettings.backgroundSecs, refreshSettings.foregroundSecs]);

  const value = useMemo(() => ({ providers, usages, errors, refreshingIds, manualRefreshingIds, refreshState, lastUpdated, historyRevision, refreshAll, refreshOne, reloadProviders }), [providers, usages, errors, refreshingIds, manualRefreshingIds, refreshState, lastUpdated, historyRevision, refreshAll, refreshOne, reloadProviders]);
  return <MonitorStoreContext.Provider value={value}>{children}</MonitorStoreContext.Provider>;
}

export function useMonitorStore() {
  const value = useContext(MonitorStoreContext);
  if (!value) throw new Error("useMonitorStore must be used inside MonitorStoreProvider");
  return value;
}
