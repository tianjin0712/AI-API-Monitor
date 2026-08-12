import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import ProviderCard from "../components/ProviderCard";
import { DEFAULT_WIDGETS, parseWidgets } from "../utils/layout";
import type {
  DashboardWidget,
  ProviderConfig,
  ProviderUsage,
  RefreshSettings,
} from "../types";

/** 刷新最小间隔（毫秒），与后端前台最小 10 秒约束一致（P2 修复） */
const MIN_INTERVAL_MS = 10_000;

/** 总览页：Widget 容器（V0.3 DIY UI）+ 刷新调度 */
export default function Dashboard({ theme }: { theme: "dark" | "light" }) {
  const [widgets, setWidgets] = useState<DashboardWidget[]>(DEFAULT_WIDGETS);
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

  // 首次加载：布局 + Provider 列表 + 刷新策略
  useEffect(() => {
    void api
      .getLayout()
      .then((json) => setWidgets(parseWidgets(json)))
      .catch(() => {});
    void loadProviders();
    void api
      .getRefreshSettings()
      .then(setRefreshSettings)
      .catch((e) => setError(String(e)));
  }, [loadProviders]);

  // 布局持久化（V0.3）：widgets 或 theme 变化后防抖保存到后端（含主题）
  const saveTimer = useRef<number | undefined>(undefined);
  useEffect(() => {
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      const json = JSON.stringify({ theme, widgets });
      void api.setLayout(json).catch(() => {});
    }, 500);
    return () => window.clearTimeout(saveTimer.current);
  }, [widgets, theme]);

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

  const visibleWidgets = widgets.filter((w) => w.visible);

  // ---- V0.3 编辑模式：拖拽排序 + 显示/隐藏 ----
  const [editing, setEditing] = useState(false);
  const [dragIndex, setDragIndex] = useState<number | null>(null);

  const toggleVisible = (id: string) => {
    setWidgets((ws) =>
      ws.map((w) => (w.id === id ? { ...w, visible: !w.visible } : w)),
    );
  };

  const moveWidget = (from: number, to: number) => {
    setWidgets((ws) => {
      const next = [...ws];
      const [moved] = next.splice(from, 1);
      next.splice(to, 0, moved);
      return next;
    });
  };

  const onDropAt = (index: number) => (e: React.DragEvent) => {
    e.preventDefault();
    if (dragIndex !== null && dragIndex !== index) moveWidget(dragIndex, index);
    setDragIndex(null);
  };

  const renderWidget = (w: DashboardWidget, index: number) => {
    const body =
      w.type === "summary" ? (
        <SummaryWidget providers={providers} usages={usages} />
      ) : w.type === "cost" ? (
        <CostWidget usages={usages} />
      ) : (
        <div className="flex flex-col gap-3">
          {providers.length === 0 ? (
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
          ) : (
            <>
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
            </>
          )}
        </div>
      );

    if (!editing) return <div key={w.id}>{body}</div>;

    return (
      <div
        key={w.id}
        draggable
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          setDragIndex(index);
        }}
        onDragOver={(e) => e.preventDefault()}
        onDrop={onDropAt(index)}
        className={`relative rounded-2xl border transition-opacity ${
          w.visible
            ? "border-dashed border-border bg-card/60"
            : "border-dashed border-border opacity-50"
        }`}
      >
        {/* 编辑工具栏 */}
        <div className="flex items-center justify-between px-3 pt-2">
          <div className="flex items-center gap-2">
            <span className="cursor-grab text-text-muted" title="拖动排序">
              ⠿
            </span>
            <span className="text-[12px] font-medium text-text-secondary">
              {w.type === "providers"
                ? "账户列表"
                : w.type === "summary"
                  ? "今日汇总"
                  : "费用概览"}
            </span>
          </div>
          <button
            className="text-[11px] text-text-secondary underline-offset-2 hover:text-text-primary hover:underline"
            onClick={() => toggleVisible(w.id)}
          >
            {w.visible ? "隐藏" : "显示"}
          </button>
        </div>
        <div className="px-3 pb-3 pt-1">{body}</div>
      </div>
    );
  };

  return (
    <div className="flex flex-col gap-3">
      {error && (
        <div className="rounded-xl border border-danger/30 bg-danger/10 px-3 py-2 text-[12px] text-danger">
          {error}
        </div>
      )}

      {/* 编辑模式入口（V0.3） */}
      <div className="flex items-center justify-end">
        <button
          className={`btn px-3 py-1 text-[12px] ${
            editing ? "btn-primary" : "btn-ghost"
          }`}
          onClick={() => setEditing((e) => !e)}
        >
          {editing ? "完成编辑" : "编辑布局"}
        </button>
      </div>

      {editing
        ? widgets.map((w, i) => renderWidget(w, i))
        : visibleWidgets.map((w, i) => renderWidget(w, i))}
    </div>
  );
}

/** 汇总 Widget：账户数 + 今日消耗合计 + Token 合计 */
function SummaryWidget({
  providers,
  usages,
}: {
  providers: ProviderConfig[];
  usages: Record<number, ProviderUsage>;
}) {
  const list = Object.values(usages);
  const todayCost = list.reduce(
    (sum, u) => sum + (u.todayCost ?? 0),
    0,
  );
  const totalTokens = list.reduce((sum, u) => sum + u.totalTokens, 0);
  return (
    <section className="glass p-4">
      <h3 className="text-[12px] font-semibold uppercase tracking-wide text-text-muted">
        今日汇总
      </h3>
      <div className="mt-3 grid grid-cols-3 gap-2 text-center">
        <SummaryStat label="账户" value={String(providers.length)} />
        <SummaryStat label="今日消耗" value={todayCost.toFixed(2)} />
        <SummaryStat label="Token" value={totalTokens.toLocaleString("zh-CN")} />
      </div>
    </section>
  );
}

/** 费用/余额 Widget：总余额 + 近 30 天费用 */
function CostWidget({ usages }: { usages: Record<number, ProviderUsage> }) {
  const list = Object.values(usages);
  const balance = list.reduce((sum, u) => sum + (u.balance ?? 0), 0);
  const monthCost = list.reduce((sum, u) => sum + (u.monthCost ?? 0), 0);
  return (
    <section className="glass p-4">
      <h3 className="text-[12px] font-semibold uppercase tracking-wide text-text-muted">
        费用概览
      </h3>
      <div className="mt-3 grid grid-cols-2 gap-2 text-center">
        <SummaryStat label="账户总余额" value={balance.toFixed(2)} />
        <SummaryStat label="近 30 天费用" value={monthCost.toFixed(2)} />
      </div>
    </section>
  );
}

function SummaryStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-white/[0.03] px-2 py-2">
      <div className="truncate text-[14px] font-semibold text-text-primary">
        {value}
      </div>
      <div className="mt-0.5 text-[10px] text-text-muted">{label}</div>
    </div>
  );
}
