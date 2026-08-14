import { useState } from "react";
import ProviderCard from "../components/ProviderCard";
import TrendWidget from "../components/TrendWidget";
import { Checkbox } from "../components/ui/Controls";
import { DEFAULT_WIDGETS } from "../utils/layout";
import { useMonitorStore } from "../state/MonitorStore";
import type { DashboardWidget, ProviderUsage } from "../types";
import { luotianyiGifPath } from "../utils/themeAssets";

interface Props {
  widgets: DashboardWidget[];
  onWidgetsChange: (updater: (ws: DashboardWidget[]) => DashboardWidget[]) => void;
  visualTheme?: "default" | "luotianyi" | "custom";
  avatarGif?: string;
}

export default function Dashboard({ widgets, onWidgetsChange, visualTheme, avatarGif }: Props) {
  const { providers, usages, errors, manualRefreshingIds, refreshAll, refreshOne, historyRevision } = useMonitorStore();
  const [editing, setEditing] = useState(false);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const visibleWidgets = widgets.filter((widget) => widget.visible);

  const renderWidget = (widget: DashboardWidget, index: number) => {
    const body = widget.type === "summary" ? <SummaryWidget providers={providers.length} usages={usages} />
      : widget.type === "cost" ? <CostWidget usages={usages} />
      : widget.type === "trend" ? <TrendWidget providers={providers} historyRevision={historyRevision} />
      : <div className="flex flex-col gap-3">
        {providers.length === 0 ? <div className="empty-state mx-card p-8 text-center"><p className="text-[15px] font-medium text-text-primary">尚未添加 Provider</p><p className="mt-2 text-[13px] text-text-secondary">前往“设置”添加 API 账户</p></div> : <>
          <div className="flex items-center justify-between gap-2"><span className="micro-glass-pill">{providers.length} 个账户</span><button className="icon-refresh-button" onClick={() => void refreshAll()} disabled={manualRefreshingIds.size > 0} aria-label="立即刷新"><span className={manualRefreshingIds.size > 0 ? "is-spinning" : "provider-refresh-icon"}>↻</span></button></div>
          {providers.map((provider) => <ProviderCard key={provider.id} provider={provider} usage={usages[provider.id]} error={errors[provider.id]} refreshing={manualRefreshingIds.has(provider.id)} onRefresh={() => void refreshOne(provider.id)} />)}
        </>}</div>;
    if (!editing) return <div key={widget.id}>{body}</div>;
    return <div key={widget.id} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); if (dragIndex !== null && dragIndex !== index) onWidgetsChange((list) => { const next = [...list]; const [moved] = next.splice(dragIndex, 1); next.splice(index, 0, moved); return next; }); setDragIndex(null); }} className="relative rounded-2xl border border-dashed border-border bg-card/60">
      <div className="flex items-center justify-between px-3 pt-2"><span draggable onDragStart={() => setDragIndex(index)} onDragEnd={() => setDragIndex(null)} className="cursor-grab text-text-muted">⋮⋮</span><Checkbox checked={widget.visible} onChange={() => onWidgetsChange((list) => list.map((item) => item.id === widget.id ? { ...item, visible: !item.visible } : item))} label="显示" /></div><div className="px-3 pb-3 pt-1">{body}</div>
    </div>;
  };

  return <div className="dashboard-panel flex flex-col gap-3">
    <section className={`welcome-hero mx-card relative flex min-h-28 items-center overflow-hidden px-4 py-3 ${visualTheme === "luotianyi" ? "luotianyi-hero" : ""}`}><div className="relative z-10 max-w-[58%]"><div className="welcome-eyebrow text-[11px] font-semibold uppercase text-accent">{visualTheme === "luotianyi" ? "LUO TIANYI" : "AI API MONITOR"}</div><h2 className="welcome-title mt-1 font-bold text-text-primary">今天也在好好管理 Token</h2><p className="mt-1 text-[11px] leading-relaxed text-text-secondary">账户状态、余额与消耗会在这里持续更新。</p></div>{avatarGif && <img className="pointer-events-none absolute bottom-1 right-2 h-[104px] w-[104px] object-contain" src={luotianyiGifPath(avatarGif)} alt="洛天依动画" />}</section>
    {(editing ? widgets : visibleWidgets).map(renderWidget)}
    <div className="flex items-center justify-end gap-2 pt-1">{editing && <button className="micro-glass-pill" onClick={() => onWidgetsChange(() => DEFAULT_WIDGETS)}>恢复默认</button>}<button className={`micro-glass-pill ${editing ? "is-active" : ""}`} onClick={() => setEditing((value) => !value)}>{editing ? "完成编辑" : "编辑布局"}</button></div>
  </div>;
}

function SummaryWidget({ providers, usages }: { providers: number; usages: Record<number, ProviderUsage> }) {
  const totalTokens = Object.values(usages).reduce((sum, usage) => sum + usage.totalTokens, 0);
  return <section className="mx-card p-4"><h3 className="text-[12px] font-semibold uppercase tracking-wide text-text-muted">今日汇总</h3><div className="mt-3 grid grid-cols-2 gap-2 text-center sm:grid-cols-3"><SummaryStat label="账户" value={String(providers)} /><SummaryStat label="Token" value={totalTokens.toLocaleString("zh-CN")} /></div></section>;
}

function CostWidget({ usages }: { usages: Record<number, ProviderUsage> }) {
  const balances = Object.values(usages).filter((usage) => usage.balance != null);
  return <section className="mx-card p-4"><h3 className="text-[12px] font-semibold uppercase tracking-wide text-text-muted">费用概览</h3><div className="mt-3 flex flex-wrap gap-2">{balances.map((usage) => <SummaryStat key={usage.providerId} label={`余额 ${usage.currency}`} value={Number(usage.balance).toFixed(2)} />)}{balances.length === 0 && <SummaryStat label="暂无数据" value="—" />}</div></section>;
}

function SummaryStat({ label, value }: { label: string; value: string }) { return <div className="widget-stat rounded-lg px-2 py-2"><div className="truncate text-[14px] font-semibold text-text-primary">{value}</div><div className="mt-0.5 text-[10px] text-text-muted">{label}</div></div>; }
