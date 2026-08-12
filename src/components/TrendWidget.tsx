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

  const load = useCallback(async () => {
    if (effectiveId === null) return;
    setError(null);
    try {
      const [h, p] = await Promise.all([
        api.getUsageHistory(effectiveId, 30),
        api.getPrediction(effectiveId),
      ]);
      setHistory(h);
      setPrediction(p);
    } catch (e) {
      setError(String(e));
    }
  }, [effectiveId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (providers.length === 0) return null;

  const values = history
    .map((d) => (metric === "tokens" ? d.tokens : d.cost))
    .filter((v) => Number.isFinite(v) && v >= 0);
  const max = Math.max(...values, 1);

  // SVG 折线路径（viewBox 100x40，比例缩放）
  const points = history.map((d, i) => {
    const v = metric === "tokens" ? d.tokens : d.cost;
    const x = history.length > 1 ? (i / (history.length - 1)) * 100 : 50;
    const y = 38 - (Math.max(0, Math.min(v, max)) / max) * 34;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  const polyline = points.join(" ");

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
            {points.map((p, i) => {
              const [x, y] = p.split(",").map(Number);
              return (
                <circle
                  key={i}
                  cx={x}
                  cy={y}
                  r="0.7"
                  fill="var(--color-accent)"
                  vectorEffect="non-scaling-stroke"
                />
              );
            })}
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
