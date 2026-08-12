import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { DailyUsage, Prediction, ProviderConfig } from "../types";

interface Props {
  providers: ProviderConfig[];
}

/** V0.5 趋势 Widget：Token/费用历史折线 + 消耗预测（mission.md §5/§13） */
export default function TrendWidget({ providers }: Props) {
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [history, setHistory] = useState<DailyUsage[]>([]);
  const [prediction, setPrediction] = useState<Prediction | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [metric, setMetric] = useState<"tokens" | "cost">("cost");

  const effectiveId =
    selectedId ?? providers[0]?.id ?? null;

  // P2：Provider 被删除后校验 selectedId，不存在则重置并清空旧数据
  useEffect(() => {
    if (selectedId !== null && !providers.some((p) => p.id === selectedId)) {
      setSelectedId(null);
      setHistory([]);
      setPrediction(null);
      setError(null);
    }
  }, [providers, selectedId]);

  const load = useCallback(async () => {
    if (effectiveId === null) return;
    setError(null);
    // P2：历史/预测独立降级（Promise.allSettled），一方失败不阻塞另一方
    const [h, p] = await Promise.allSettled([
      api.getUsageHistory(effectiveId, 30),
      api.getPrediction(effectiveId),
    ]);
    if (h.status === "fulfilled") setHistory(h.value);
    else setError(`历史加载失败: ${h.reason}`);
    if (p.status === "fulfilled") setPrediction(p.value);
    else setError((e) => (e ? `${e}；预测加载失败: ${p.reason}` : `预测加载失败: ${p.reason}`));
  }, [effectiveId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (providers.length === 0) return null;

  // P2：统一生成已校验数据点（拒绝 NaN/负值/未知），max 与绘制共用同一集合
  const series = history
    .map((d, i) => {
      const v = metric === "tokens" ? d.todayTokens : d.cost;
      if (v === null || v === undefined || !Number.isFinite(v) || v < 0) return null;
      return { index: i, value: v };
    })
    .filter((p): p is { index: number; value: number } => p !== null);
  const max = series.length > 0 ? Math.max(...series.map((p) => p.value), 1) : 1;
  const points = series.map(({ index, value }) => {
    const x = history.length > 1 ? (index / (history.length - 1)) * 100 : 50;
    const y = 38 - (Math.min(value, max) / max) * 34;
    return { x, y };
  });
  const polyline = points.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");

  return (
    <section className="glass p-4">
      <div className="flex items-center justify-between">
        <h3 className="text-[12px] font-semibold uppercase tracking-wide text-text-muted">
          消耗趋势
        </h3>
        <div className="flex items-center gap-1.5">
          {providers.length > 1 && (
            <select
              className="input px-2 py-0.5 text-[11px]"
              value={effectiveId ?? ""}
              onChange={(e) => setSelectedId(Number(e.target.value))}
            >
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          )}
          <select
            className="input px-2 py-0.5 text-[11px]"
            value={metric}
            onChange={(e) => setMetric(e.target.value as "tokens" | "cost")}
          >
            <option value="cost">费用</option>
            <option value="tokens">Token</option>
          </select>
        </div>
      </div>

      {error && (
        <p className="mt-2 text-[11px] text-danger">{error}</p>
      )}

      <div className="mt-2">
        {history.length >= 2 ? (
          <svg viewBox="0 0 100 40" className="h-24 w-full" preserveAspectRatio="none">
            <polyline
              points={polyline}
              fill="none"
              stroke="var(--color-accent)"
              strokeWidth="0.8"
              vectorEffect="non-scaling-stroke"
            />
            {points.map((p, i) => (
              <circle
                key={i}
                cx={p.x}
                cy={p.y}
                r="0.7"
                fill="var(--color-accent)"
                vectorEffect="non-scaling-stroke"
              />
            ))}
          </svg>
        ) : (
          <p className="py-6 text-center text-[11px] text-text-muted">
            历史数据不足（至少需要 2 天记录，持续使用后自动生成）
          </p>
        )}
      </div>

      {/* 预测（mission.md §13） */}
      {prediction && (
        <div className="mt-2 flex flex-wrap gap-2 text-center">
          <Mini label="日均消耗" value={prediction.dailyCostAvg.toFixed(2)} />
          <Mini
            label="预计剩余"
            value={
              prediction.daysLeft !== null
                ? `${prediction.daysLeft.toFixed(1)} 天`
                : "—"
            }
          />
          <Mini
            label="预计耗尽"
            value={prediction.exhaustedDate ?? "—"}
          />
        </div>
      )}
      {prediction && prediction.samples > 0 && (
        <p className="mt-1.5 text-[10px] text-text-muted">
          基于近 {prediction.daysSpan} 天中 {prediction.samples} 个有效费用样本。
        </p>
      )}
      {!prediction && !error && history.length > 0 && (
        <p className="mt-2 text-[10px] text-text-muted">
          当前账户无余额或日均消耗为 0，暂无法预测耗尽时间。
        </p>
      )}
    </section>
  );
}

function Mini({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex-1 rounded-lg bg-white/[0.03] px-2 py-1.5">
      <div className="truncate text-[13px] font-semibold text-text-primary">
        {value}
      </div>
      <div className="mt-0.5 text-[10px] text-text-muted">{label}</div>
    </div>
  );
}
