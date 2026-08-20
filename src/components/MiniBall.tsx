import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../api";
import type { WindowMode } from "../types";
import { formatCount, formatMoney } from "../utils/format";
import { luotianyiGifPath } from "../utils/themeAssets";
import { useMonitorStore } from "../state/MonitorStore";

interface Props {
  mode: WindowMode;
  onExpand: () => void;
  /** mini 模式显示为横向紧凑条 */
  compact?: boolean;
  visualTheme?: "default" | "luotianyi" | "custom";
  avatarGif?: string;
  backgroundImage?: string | null;
  floatingScrollMode?: "auto" | "wheel";
  onSwitchMode: (mode: WindowMode) => void;
}

/**
 * Mini 紧凑条 / Ball 悬浮小球（V0.2）。
 * 无边框窗口 + data-tauri-drag-region 实现拖动；点击展开回 Full。
 */
export default function MiniBall({ onExpand, compact, visualTheme, avatarGif, backgroundImage, floatingScrollMode = "auto", onSwitchMode }: Props) {
  const { providers, usages } = useMonitorStore();
  const loading = false;
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [hovered, setHovered] = useState(false);
  const [shapeExiting, setShapeExiting] = useState(false);
  const collapseTimer = useRef<number | undefined>(undefined);
  const hoverTimer = useRef<number | undefined>(undefined);
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);
  const collapseAfterDrag = useRef(false);
  const lastDragAt = useRef(0);
  const clickTimer = useRef<number | undefined>(undefined);
  const shapeTimer = useRef<number | undefined>(undefined);
  const tooltipTimer = useRef<number | undefined>(undefined);
  const [tooltipVisible, setTooltipVisible] = useState(false);
  const sharedBackground = backgroundImage
    ? `linear-gradient(110deg, color-mix(in srgb, var(--surface-raised) 90%, transparent), color-mix(in srgb, var(--color-accent) 18%, var(--surface-raised))), url(${backgroundImage}) center / cover`
    : undefined;

  useEffect(() => () => {
    window.clearTimeout(hoverTimer.current);
    window.clearTimeout(collapseTimer.current);
    window.clearTimeout(clickTimer.current);
    window.clearTimeout(shapeTimer.current);
    window.clearTimeout(tooltipTimer.current);
    setTooltipVisible(false);
  }, []);

  const showTooltipLater = () => {
    window.clearTimeout(tooltipTimer.current);
    tooltipTimer.current = window.setTimeout(() => {
      setTooltipVisible(true);
    }, 1000);
  };
  const hideTooltip = () => {
    window.clearTimeout(tooltipTimer.current);
    setTooltipVisible(false);
  };

  useEffect(() => {
    setShapeExiting(false);
  }, [compact]);

  const handleShapeClick = (event: React.MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest("button, input, select, label")) return;
    if (dragging.current || Date.now() - lastDragAt.current < 300) { dragging.current = false; return; }
    window.clearTimeout(clickTimer.current);
    clickTimer.current = window.setTimeout(() => {
      setShapeExiting(true);
      shapeTimer.current = window.setTimeout(() => onSwitchMode(compact ? "ball" : "mini"), 140);
    }, 220);
  };
  const handleShapeDoubleClick = (event: React.MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest("button, input, select, label")) return;
    window.clearTimeout(clickTimer.current);
    window.clearTimeout(shapeTimer.current);
    setShapeExiting(false);
    // Only the expanded bar uses double-click to open the main window.
    // A double-click on the square still performs its single intended action once.
    if (compact) onExpand();
    else onSwitchMode("mini");
  };

  const beginPotentialDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button, input, select, label")) return;
    window.clearTimeout(hoverTimer.current);
    window.clearTimeout(collapseTimer.current);
    hideTooltip();
    dragStart.current = { x: event.screenX, y: event.screenY };
    dragging.current = false;
  };

  const continuePotentialDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const start = dragStart.current;
    if (!start || dragging.current || (event.buttons & 1) === 0) return;
    if (Math.hypot(event.screenX - start.x, event.screenY - start.y) < 4) return;
    dragging.current = true;
    lastDragAt.current = Date.now();
    dragStart.current = null;
    void getCurrentWindow().startDragging()
      .catch((error) => console.error("悬浮窗拖动失败:", error))
      .finally(() => {
        dragging.current = false;
        collapseAfterDrag.current = false;
        // Native dragging can finish one or two compositor frames after the
        // pointer event. Retry after the position settles so all four edges
        // (including right/bottom taskbar edges) use the final coordinates.
        const snap = () => void api.snapWindowToWorkArea().catch((error) => console.error("悬浮窗边缘吸附失败:", error));
        snap();
        window.setTimeout(snap, 90);
        window.setTimeout(snap, 220);
      });
  };

  const endPotentialDrag = () => {
    dragStart.current = null;
  };

  useEffect(() => {
    setSelectedId((current) => current ?? providers[0]?.id ?? null);
  }, [providers]);

  useEffect(() => {
    const cycleProviders = providers.filter((provider) => usages[provider.id]);
    if (!compact || floatingScrollMode !== "auto" || cycleProviders.length < 2 || hovered) return;
    const timer = window.setInterval(() => {
      setSelectedId((current) => {
        const index = Math.max(0, cycleProviders.findIndex((provider) => provider.id === current));
        return cycleProviders[(index + 1) % cycleProviders.length]?.id ?? current;
      });
    }, 4000);
    return () => window.clearInterval(timer);
  }, [compact, floatingScrollMode, hovered, providers, usages]);

  const displayProviders = providers.filter((provider) => usages[provider.id]);
  const availableProviders = displayProviders.length > 0 ? displayProviders : providers;
  const selectedIndex = Math.max(0, availableProviders.findIndex((provider) => provider.id === selectedId));
  const selected = availableProviders[selectedIndex];
  const usage = selected ? usages[selected.id] : undefined;
  const codexTokenRemaining = usage?.codex?.windows.find((window) => window.tokensRemaining != null)?.tokensRemaining;
  const isCodexTokenQuota = selected?.providerType === "codex" && codexTokenRemaining != null;
  const value = isCodexTokenQuota
    ? `${formatCount(codexTokenRemaining!)} Token`
    : usage?.remaining != null
      ? `${formatMoney(usage.remaining)}${usage.custom ? customUnitSuffix(usage.custom.unit) : "%"}`
    : usage?.balance != null
      ? `${formatMoney(usage.balance)} ${usage.currency}`
      : usage
        ? `${formatCount(usage.totalTokens)} 已用`
        : loading ? "刷新中…" : "暂无数据";

  const cycleProvider = (direction: number) => {
    if (availableProviders.length < 2) return;
    const next = (selectedIndex + direction + availableProviders.length) % availableProviders.length;
    setSelectedId(availableProviders[next].id);
  };

  if (compact) {
    // Mini：横向紧凑条
    return (
      <div
        onClick={handleShapeClick}
        onDoubleClick={handleShapeDoubleClick}
        onPointerDown={beginPotentialDrag}
        onPointerMove={continuePotentialDrag}
        onPointerUp={endPotentialDrag}
        onPointerCancel={endPotentialDrag}
        onPointerEnter={() => {
          setHovered(true);
          showTooltipLater();
          collapseAfterDrag.current = false;
          window.clearTimeout(collapseTimer.current);
        }}
        onPointerLeave={() => { setHovered(false); hideTooltip(); }}
        onWheel={(event) => {
          event.stopPropagation();
          if (floatingScrollMode === "wheel") cycleProvider(event.deltaY > 0 ? 1 : -1);
        }}
        className={`mini-monitor glass flex h-full cursor-pointer items-center ${shapeExiting ? "is-shape-exiting" : ""} ${visualTheme === "luotianyi" ? "is-luotianyi" : "px-3"}`}
        aria-label="单击收起，双击展开"
        style={sharedBackground ? { "--floating-content-background": sharedBackground } as CSSProperties : undefined}
      >
        {(visualTheme === "luotianyi" || avatarGif) && (
          <div className="mini-avatar-frame shrink-0"><img className="mini-avatar-image" src={loading ? "/themes/luotianyi/loading.gif" : luotianyiGifPath(avatarGif)} alt="洛天依" /></div>
        )}
        <div className="mini-drawer-content min-w-0 flex-1 px-2">
          <div className="mini-provider-module" key={selected?.id ?? "empty"}>
            <div className="mini-provider-name truncate">{selected?.name ?? "尚未配置账户"}</div>
            <div className="mini-balance-row mt-1 flex items-baseline gap-1"><span className="truncate text-[16px] font-bold">{value}</span><span className="mini-balance-label shrink-0 text-[9px]">{isCodexTokenQuota ? "剩余 Token" : "剩余额度"}</span></div>
            <div className="mini-balance-meta mt-0.5 text-[9px]">{usage?.totalTokens ? `${formatCount(usage.totalTokens)} Token` : "额度信息"}{providers.length > 1 ? ` · ${selectedIndex + 1}/${providers.length}` : ""}</div>
          </div>
        </div>
        <div className={`floating-inline-tooltip ${tooltipVisible ? "is-visible" : ""}`} aria-hidden="true">单击收起 · 双击展开</div>
      </div>
    );
  }

  // Ball：圆角方块（96×96 窗口，与 Mini 等高）
  return (
    <div
      onClick={handleShapeClick}
      onDoubleClick={handleShapeDoubleClick}
      onPointerDown={beginPotentialDrag}
      onPointerMove={continuePotentialDrag}
      onPointerUp={endPotentialDrag}
      onPointerCancel={endPotentialDrag}
      onMouseEnter={() => { setHovered(true); showTooltipLater(); }}
      onMouseLeave={() => { setHovered(false); hideTooltip(); }}
      className={`floating-cube flex h-full w-full cursor-pointer items-center justify-start ${hovered ? "is-hovered" : ""} ${shapeExiting ? "is-shape-exiting" : ""} ${visualTheme === "luotianyi" ? "luotianyi-ball" : ""}`}
      aria-label="单击展开"
      style={{
        ...(sharedBackground ? { "--floating-content-background": sharedBackground } : {}),
      } as CSSProperties}
    >
      {visualTheme === "luotianyi" || avatarGif ? <div className="ball-avatar-frame"><img className="pointer-events-none mini-avatar-image" src={luotianyiGifPath(avatarGif)} alt="洛天依悬浮图标" /></div> : <span
        className="h-2 w-2 rounded-full"
        style={{
          background: "var(--color-accent)",
          boxShadow: "0 0 8px var(--color-accent)",
        }}
      />}
      <div className={`floating-inline-tooltip ${tooltipVisible ? "is-visible" : ""}`} aria-hidden="true">单击展开</div>
      </div>
  );
}

function customUnitSuffix(unit: string): string {
  switch (unit) {
    case "token": return " Token";
    case "count": return " 次";
    case "currency": return "";
    default: return "";
  }
}
