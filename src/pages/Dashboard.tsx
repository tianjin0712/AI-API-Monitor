import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import ProviderCard from "../components/ProviderCard";
import type {
  ProviderConfig,
  ProviderUsage,
  RefreshSettings,
} from "../types";

/** 刷新最小间隔（毫秒），与后端前台最小 10 秒约束一致（P2 修复） */
const MIN_INTERVAL_MS = 10_000;

/** 总览页：Provider 卡片列表 + 手动刷新 + 前台/后台轮询（单飞） */
export default function Dashboard() {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [usages, setUsages] = useState<Record<number, ProviderUsage>>({});
  const [refreshingIds, setRefreshingIds] = useState<Set<number>>(new Set());
  const [errors, setErrors] = useState<Record<number, string>>({});
  const [refreshSettings, setRefreshSettings] = useState<RefreshSettings>({
    foregroundSecs: 10,
    backgroundSecs: 60,
  });
  const [error, setError] = useState<string | null>(null);

  // 单飞控制：同一时刻只允许一个刷新任务（修复 P1 并发竞态）
  const refreshingRef = useRef(false);
  const refreshAllRef = useRef<(p: ProviderConfig[]) => Promise<void>>(
    async () => {},
  );

  const loadProviders = useCallback(async () => {
    try {
      setProviders(await api.listProviders());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const refreshAll = useCallback(async (prov: ProviderConfig[]) => {
    const ids = prov.map((p) => p.id);
    if (ids.length === 0) return;
    setRefreshingIds(new Set(ids));
    setError(null);
    try {
      const list = await api.refreshAll();
      setUsages((prev) => {
        const next = { ...prev };
        for (const r of list) {
          if (r.success && r.usage?.providerId != null) {
            next[r.usage.providerId] = r.usage;
          }
        }
        return next;
      });
      setErrors((prev) => {
        const next = { ...prev };
        for (const r of list) {
          if (r.success) delete next[r.providerId];
          else next[r.providerId] = r.error ?? "刷新失败";
        }
        return next;
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshingIds(new Set());
    }
  }, []);

  // providers 的 ref 版本（避免 runRefresh 依赖 providers 而重建）
  const providersRef = useRef(providers);
  useEffect(() => {
    providersRef.current = providers;
  }, [providers]);

  // 用 ref 保持最新 refreshAll（调度 effect 不因它重建）
  useEffect(() => {
    refreshAllRef.current = refreshAll;
  }, [refreshAll]);

  // 单飞入口：调度与手动刷新共用
  const runRefresh = useCallback(async () => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    try {
      await refreshAllRef.current(providersRef.current);
    } finally {
      refreshingRef.current = false;
    }
  }, []);

  // 单卡片刷新（独立状态，与批量共享单飞）
  const refreshOne = useCallback(
    async (id: number) => {
      if (refreshingRef.current) return;
      refreshingRef.current = true;
      setRefreshingIds(new Set([id]));
      try {
        const usage = await api.refreshProvider(id);
        setUsages((prev) => ({ ...prev, [id]: usage }));
        setErrors((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
      } catch (e) {
        setErrors((prev) => ({ ...prev, [id]: String(e) }));
      } finally {
        setRefreshingIds(new Set());
        refreshingRef.current = false;
      }
    },
    [],
  );

  // 首次加载：读取 Provider 列表与刷新策略
  useEffect(() => {
    void loadProviders();
    void api
      .getRefreshSettings()
      .then(setRefreshSettings)
      .catch((e) => setError(String(e)));
  }, [loadProviders]);

  // Provider 列表变化后自动刷新一次
  useEffect(() => {
    if (providers.length > 0) void runRefresh();
  }, [providers.length]); // eslint-disable-line react-hooks/exhaustive-deps

  // 调度器：前台/后台间隔（mission.md §12），窗口聚焦立即刷新。
  // setTimeout 链式调度，visibilitychange/focus 重置；单飞防重叠。
  useEffect(() => {
    const intervalOf = () =>
      (document.visibilityState === "visible"
        ? Math.max(refreshSettings.foregroundSecs, 10)
        : Math.max(refreshSettings.backgroundSecs, 60)) * 1000;

    let timer: number;
    const tick = () => {
      void runRefresh();
      timer = window.setTimeout(tick, Math.max(MIN_INTERVAL_MS, intervalOf()));
    };
    timer = window.setTimeout(tick, Math.max(MIN_INTERVAL_MS, intervalOf()));

    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(tick, Math.max(MIN_INTERVAL_MS, intervalOf()));
    };
    const onFocus = () => {
      reset();
      void runRefresh();
    };
    document.addEventListener("visibilitychange", reset);
    window.addEventListener("focus", onFocus);
    // 后端窗口聚焦事件（系统唤醒/窗口恢复时触发，比 WebView focus 更可靠）
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<void>("app-focused", () => {
      reset();
      void runRefresh();
    }).then((fn) => {
      // 组件在 promise resolve 前卸载时立即注销，避免 listener 泄漏
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", reset);
      window.removeEventListener("focus", onFocus);
      cancelled = true;
      unlisten?.();
    };
  }, [refreshSettings.foregroundSecs, refreshSettings.backgroundSecs, runRefresh]);

  if (providers.length === 0 && !error) {
    return (
      <div className="animate-fade-in-up glass mt-8 p-8 text-center">
        <p className="text-[15px] font-medium text-text-primary">
          尚未添加 Provider
        </p>
        <p className="mt-2 text-[13px] text-text-secondary">
          前往「设置」添加 DeepSeek / OpenAI 等 API 账户，
          <br />
          即可在此查看余额与 Token 消耗
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {error && (
        <div className="rounded-xl border border-danger/30 bg-danger/10 px-3 py-2 text-[12px] text-danger">
          {error}
        </div>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[12px] text-text-muted">
          {providers.length} 个账户
        </span>
        <button
          className="btn btn-ghost px-3 py-1 text-[12px]"
          onClick={() => void runRefresh()}
          disabled={refreshingIds.size > 0}
        >
          {refreshingIds.size > 0 ? "刷新中…" : "立即刷新"}
        </button>
      </div>

      {providers.map((p) => (
        <ProviderCard
          key={p.id}
          provider={p}
          usage={usages[p.id]}
          error={errors[p.id]}
          refreshing={refreshingIds.has(p.id)}
          onRefresh={() => void refreshOne(p.id)}
        />
      ))}
    </div>
  );
}
