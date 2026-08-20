import type {
  CodexRateLimitWindow,
  ProviderConfig,
  ProviderUsage,
} from "../types";
import { formatCount, formatMoney, formatRelativeTime } from "../utils/format";

interface Props {
  provider: ProviderConfig;
  usage?: ProviderUsage;
  error?: string;
  refreshing: boolean;
  onRefresh?: () => void;
}

export default function ProviderCard({ provider, usage, error, refreshing, onRefresh }: Props) {
  const balance = usage?.balance ?? null;
  const remaining = usage?.remaining ?? null;
  const todayCost = usage?.todayCost ?? null;
  const currency = usage?.currency || "¥";
  const isCodex = provider.providerType === "codex";
  const custom = usage?.custom ?? null;
  const isCustom = custom !== null;
  const percent = remaining !== null && Number.isFinite(remaining)
    ? Math.max(0, Math.min(100, remaining))
    : balance !== null ? 100 : 0;
  const barTone = percent >= 50 ? "success" : percent >= 30 ? "warning" : "danger";

  return (
    <div className={`provider-card mx-card animate-fade-in-up relative overflow-hidden transition-colors ${refreshing ? "opacity-80" : ""}`}>
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[15px] font-semibold text-text-primary">{provider.name}</span>
          <span className="provider-badge">{provider.providerType}</span>
        </div>
        {onRefresh ? (
          <button
            aria-label="刷新此账户"
            title="刷新此账户"
            disabled={refreshing}
            onClick={(event) => { event.stopPropagation(); onRefresh(); }}
            className="icon-action flex h-7 w-7 items-center justify-center text-text-muted transition-colors"
          >
            <span className={`provider-refresh-icon ${refreshing ? "is-spinning" : ""}`} aria-hidden="true">↻</span>
          </button>
        ) : null}
      </div>

      {isCodex ? <CodexQuota usage={usage} error={error} /> : (
        <>
          <div className="mt-3 flex items-baseline gap-1.5">
            <span className="provider-balance text-text-primary">
              {balance !== null ? formatMoney(balance) : usage ? "—" : "未刷新"}
            </span>
            {balance !== null && <span className="text-[13px] text-text-secondary">{currency}</span>}
            {remaining !== null && <span className="ml-auto text-[12px] text-text-secondary">剩余 {formatMoney(remaining)}{isCustom ? ` ${customUnitLabel(custom!.unit)}` : "%"}</span>}
          </div>
          <div className="provider-progress mt-2"><span data-tone={barTone} style={{ width: `${percent}%` }} /></div>
          <div className="mt-3 grid grid-cols-3 gap-2 text-center">
            <Stat label={isCustom ? "已用" : "Token"} value={isCustom ? (custom!.used != null ? formatCount(custom!.used) : "—") : formatCount(usage?.totalTokens ?? 0)} />
            <Stat label="今日消费" value={todayCost != null ? formatMoney(todayCost) : "—"} suffix={todayCost != null ? currency : ""} />
            <Stat label="更新时间" value={formatRelativeTime(usage?.updatedAt)} />
          </div>
        </>
      )}

      {error && !isCodex && (
        <div className="mt-2 flex items-start gap-1.5 rounded-lg border border-danger/25 bg-danger/10 px-2 py-1.5 text-[11px] text-danger">
          <span className="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-danger" />
          <span className="line-clamp-2 break-all">{error}</span>
        </div>
      )}
    </div>
  );
}

function CodexQuota({ usage, error }: { usage?: ProviderUsage; error?: string }) {
  const details = usage?.codex;
  const state = error ? codexErrorLabel(error) : null;
  return (
    <div className="codex-quota mt-3">
      {state ? <CodexState title={state.title} detail={state.detail} tone="danger" />
        : !usage ? <CodexState title="等待读取额度" detail="刷新后将显示 ChatGPT Codex 的实际额度窗口。" />
        : !details || details.windows.length === 0 ? <CodexState title="暂无额度信息" detail="当前账户未返回可显示的额度窗口，不会按 0% 处理。" />
        : <>
          <div className="codex-quota-meta">
            <div><span>Plan</span><strong>{details.planType || "未提供"}</strong></div>
            <div><span>Credits</span><strong>{formatCredits(details.credits)}</strong></div>
          </div>
          <div className="codex-quota-windows" aria-label="Codex 额度窗口">
            {details.windows.map((window, index) => <QuotaRing key={`${window.limitId || window.windowKind}-${index}`} window={window} />)}
          </div>
          <div className="codex-quota-updated">更新于 {formatRelativeTime(usage.updatedAt)}</div>
        </>}
    </div>
  );
}

function QuotaRing({ window }: { window: CodexRateLimitWindow }) {
  const hasTokenQuota = window.tokensRemaining != null && window.tokenLimit != null && window.tokensUsed != null;
  const remaining = Number.isFinite(window.remainingPercent)
    ? Math.max(0, Math.min(100, window.remainingPercent))
    : Math.max(0, Math.min(100, 100 - window.usedPercent));
  const radius = 30;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = window.unlimited ? 0 : circumference * (1 - remaining / 100);
  const tone = window.unlimited ? "unlimited" : remaining <= 0 ? "empty" : remaining < 30 ? "low" : "normal";
  return (
    <div className="codex-quota-window" data-tone={tone}>
      <div className="codex-quota-ring">
        <svg viewBox="0 0 72 72" aria-hidden="true">
          <circle className="codex-quota-track" cx="36" cy="36" r={radius} />
          <circle className="codex-quota-value" cx="36" cy="36" r={radius} style={{ strokeDasharray: circumference, strokeDashoffset: dashOffset }} />
        </svg>
        <div className="codex-quota-number">
            <strong>{hasTokenQuota ? formatTokenCount(window.tokensRemaining!) : window.unlimited ? "∞" : `${Math.round(remaining)}%`}</strong>
            <span>{hasTokenQuota ? "剩余 Token" : window.unlimited ? "无限" : remaining <= 0 ? "已耗尽" : "剩余"}</span>
        </div>
      </div>
      <strong className="codex-quota-name">{quotaWindowName(window)}</strong>
      <span className="codex-quota-reset">{formatReset(window.resetsAt)}</span>
    </div>
  );
}

function formatTokenCount(value: number) {
  return value.toLocaleString("zh-CN");
}

function CodexState({ title, detail, tone = "neutral" }: { title: string; detail: string; tone?: "neutral" | "danger" }) {
  return <div className="codex-quota-state" data-tone={tone}><span className="codex-quota-state-dot" /><div><strong>{title}</strong><p>{detail}</p></div></div>;
}

function quotaWindowName(window: CodexRateLimitWindow) {
  if (window.limitName) return window.limitName;
  const minutes = window.windowDurationMins;
  if (minutes) {
    if (minutes % 1440 === 0) return `${minutes / 1440} 天窗口`;
    if (minutes % 60 === 0) return `${minutes / 60} 小时窗口`;
    return `${minutes} 分钟窗口`;
  }
  return window.windowKind || window.limitId || "额度窗口";
}

function formatReset(timestamp: number | null) {
  if (!timestamp) return "重置时间未提供";
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return "重置时间未提供";
  return `重置 ${new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date)}`;
}

function formatCredits(credits: unknown) {
  if (credits == null) return "未提供";
  if (typeof credits === "string" || typeof credits === "number") return String(credits);
  if (typeof credits === "object") {
    const value = credits as Record<string, unknown>;
    const amount = value.balance ?? value.remaining ?? value.amount ?? value.value;
    const unit = value.currency ?? value.unit ?? "";
    if (typeof amount === "string" || typeof amount === "number") return `${amount}${unit ? ` ${unit}` : ""}`;
  }
  return "可用";
}

function codexErrorLabel(error: string) {
  const normalized = error.toLowerCase();
  if (/未安装|找不到|not found|runtime/.test(normalized)) return { title: "未发现 Codex Desktop Runtime", detail: "请确认 ChatGPT/Codex Desktop 已正确安装。" };
  if (/未登录|login|unauthorized|authentication/.test(normalized)) return { title: "ChatGPT 尚未登录", detail: "请先在官方 Desktop 客户端完成登录。" };
  if (/网络|network|连接|timeout|timed out/.test(normalized)) return { title: "网络连接失败", detail: "额度读取暂时不可用，可稍后手动刷新。" };
  return { title: "额度读取失败", detail: error };
}

function customUnitLabel(unit: string): string {
  switch (unit) {
    case "token": return "Token";
    case "count": return "次";
    case "currency": return "";
    default: return "";
  }
}

function Stat({ label, value, suffix }: { label: string; value: string; suffix?: string }) {
  return <div className="widget-stat rounded-lg px-2 py-1.5"><div className="truncate text-[12px] font-medium text-text-primary">{value}{suffix && <span className="ml-0.5 text-[10px] text-text-muted">{suffix}</span>}</div><div className="mt-0.5 text-[10px] text-text-muted">{label}</div></div>;
}
