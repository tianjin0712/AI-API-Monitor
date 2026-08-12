import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import ProviderCard from "../components/ProviderCard";
import type { ProviderConfig, ProviderUsage, RefreshSettings } from "../types";

/** 总览页：Provider 卡片列表 + 手动刷新 + 前台轮询 */
export default function Dashboard() {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [usages, setUsages] = useState<Record<number, ProviderUsage>>({});
  const [refreshingIds, setRefreshingIds] = useState<Set<number>>(new Set());
  const [refreshSettings, setRefreshSettings] = useState<RefreshSettings>({
    foregroundSecs: 10,
    backgroundSecs: 60,
  });
  const [error, setError] = useState<string | null>(null);

  const loadProviders = useCallback(async () => {
    try {
      setProviders(await api.listProviders());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const refreshAll = useCallback(async () => {
    const ids = providers.map((p) => p.id);
    if (ids.length === 0) return;
    setRefreshingIds(new Set(ids));
    setError(null);
    try {
      const list = await api.refreshAll();
      setUsages((prev) => {
        const next = { ...prev };
        for (const u of list) {
          if (u.providerId !== null) next[u.providerId] = u;
        }
        return next;
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshingIds(new Set());
    }
  }, [providers]);

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
    if (providers.length > 0) void refreshAll();
  }, [providers.length]); // eslint-disable-line react-hooks/exhaustive-deps

  // 调度器：前台 10s / 后台 60s（mission.md §12），窗口聚焦立即刷新
  // setTimeout 链式调度，visibilitychange/focus 时重置间隔
  useEffect(() => {
    const intervalOf = () =>
      (document.visibilityState === "visible"
        ? refreshSettings.foregroundSecs
        : refreshSettings.backgroundSecs) * 1000;

    let timer: number;
    const tick = () => {
      if (refreshingIds.size === 0) void refreshAll();
      timer = window.setTimeout(tick, Math.max(5, intervalOf()));
    };
    timer = window.setTimeout(tick, Math.max(5, intervalOf()));

    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(tick, Math.max(5, intervalOf()));
    };
    const onFocus = () => {
      reset();
      if (refreshingIds.size === 0) void refreshAll();
    };
    document.addEventListener("visibilitychange", reset);
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", reset);
      window.removeEventListener("focus", onFocus);
    };
  }, [refreshSettings.foregroundSecs, refreshSettings.backgroundSecs, refreshAll, refreshingIds.size]);

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
          onClick={() => void refreshAll()}
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
          refreshing={refreshingIds.has(p.id)}
        />
      ))}
    </div>
  );
}
