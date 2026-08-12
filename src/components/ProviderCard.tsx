import type { ProviderConfig, ProviderUsage } from "../types";
import { formatCount, formatMoney, formatRelativeTime } from "../utils/format";

interface Props {
  provider: ProviderConfig;
  usage?: ProviderUsage;
  error?: string;
  refreshing: boolean;
}

/** Dashboard 上的单个 Provider 状态卡片 */
export default function ProviderCard({ provider, usage, error, refreshing }: Props) {
  const balance = usage?.balance ?? null;
  const remaining = usage?.remaining ?? null;
  const todayCost = usage?.todayCost ?? null;
  const currency = usage?.currency || "¥";

  // 进度条：remaining 有值用百分比；否则有余额视为已连接（装饰性满条）
  const percent =
    remaining !== null && Number.isFinite(remaining)
      ? Math.max(0, Math.min(100, remaining))
      : balance !== null
        ? 100
        : 0;
  const barColor =
    percent >= 50
      ? "var(--color-success)"
      : percent >= 30
        ? "var(--color-warning)"
        : "var(--color-danger)";

  return (
    <div
      className={`glass animate-fade-in-up relative overflow-hidden p-4 transition-colors hover:border-white/15 ${
        refreshing ? "opacity-80" : ""
      }`}
    >
      {/* 头部：名称 + 类型徽标 + 状态 */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[15px] font-semibold text-text-primary">
            {provider.name}
          </span>
          <span className="rounded-md bg-white/5 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-text-secondary">
            {provider.providerType}
          </span>
        </div>
        {refreshing && (
          <span className="animate-pulse-soft h-2 w-2 rounded-full bg-accent" />
        )}
      </div>

      {/* 余额 */}
      <div className="mt-3 flex items-baseline gap-1.5">
        <span className="text-2xl font-bold tracking-tight text-text-primary">
          {balance !== null
            ? `${formatMoney(balance)}`
            : usage
              ? "—"
              : "未刷新"}
        </span>
        {balance !== null && (
          <span className="text-[13px] text-text-secondary">{currency}</span>
        )}
        {remaining !== null && (
          <span className="ml-auto text-[12px] text-text-secondary">
            剩余 {formatMoney(remaining)}%
          </span>
        )}
      </div>

      {/* 进度条 */}
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-white/5">
        <div
          className="h-full rounded-full transition-all duration-500"
          style={{ width: `${percent}%`, background: barColor }}
        />
      </div>

      {/* 数据行 */}
      <div className="mt-3 grid grid-cols-3 gap-2 text-center">
        <Stat label="Token" value={formatCount(usage?.totalTokens ?? 0)} />
        <Stat
          label="今日消耗"
          value={
            todayCost !== null && todayCost !== undefined
              ? `${formatMoney(todayCost)}`
              : "—"
          }
          suffix={todayCost !== null && todayCost !== undefined ? currency : ""}
        />
        <Stat label="更新时间" value={formatRelativeTime(usage?.updatedAt)} />
      </div>

      {/* 失败/过期状态 */}
      {error && (
        <div className="mt-2 flex items-start gap-1.5 rounded-lg border border-danger/25 bg-danger/10 px-2 py-1.5 text-[11px] text-danger">
          <span className="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-danger" />
          <span className="line-clamp-2 break-all">{error}</span>
        </div>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  suffix,
}: {
  label: string;
  value: string;
  suffix?: string;
}) {
  return (
    <div className="rounded-lg bg-white/[0.03] px-2 py-1.5">
      <div className="truncate text-[12px] font-medium text-text-primary">
        {value}
        {suffix && (
          <span className="ml-0.5 text-[10px] text-text-muted">{suffix}</span>
        )}
      </div>
      <div className="mt-0.5 text-[10px] text-text-muted">{label}</div>
    </div>
  );
}
