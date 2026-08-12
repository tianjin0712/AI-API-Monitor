import type { WindowMode } from "../types";

interface Props {
  mode: WindowMode;
  onExpand: () => void;
  /** mini 模式显示为横向紧凑条 */
  compact?: boolean;
}

/**
 * Mini 紧凑条 / Ball 悬浮小球（V0.2）。
 * 无边框窗口 + data-tauri-drag-region 实现拖动；点击展开回 Full。
 */
export default function MiniBall({ onExpand, compact }: Props) {

  if (compact) {
    // Mini：横向紧凑条
    return (
      <div
        data-tauri-drag-region
        onClick={onExpand}
        className="glass flex h-full cursor-pointer items-center justify-between px-3"
        title="点击展开"
      >
        <div data-tauri-drag-region className="flex items-center gap-2">
          <span
            className="inline-block h-2 w-2 rounded-full"
            style={{ background: "var(--color-accent)" }}
          />
          <span className="text-[13px] font-semibold text-text-primary">
            AI API Monitor
          </span>
        </div>
        <button
          aria-label="展开"
          onClick={(e) => {
            e.stopPropagation();
            onExpand();
          }}
          className="flex h-6 w-6 items-center justify-center rounded-md text-text-secondary hover:bg-white/10 hover:text-text-primary"
        >
          <svg width="10" height="10" viewBox="0 0 12 12">
            <path d="M2 4l4 4 4-4" stroke="currentColor" strokeWidth="1.4" fill="none" />
          </svg>
        </button>
      </div>
    );
  }

  // Ball：圆形小球（72×72 窗口）
  return (
    <div
      data-tauri-drag-region
      onClick={onExpand}
      className="flex h-full w-full cursor-pointer items-center justify-center rounded-full border border-white/10"
      style={{
        background:
          "radial-gradient(120% 120% at 30% 20%, rgba(108,140,255,0.35), rgba(15,17,21,0.95))",
        boxShadow: "0 4px 20px rgba(0,0,0,0.45)",
      }}
      title="AI API Monitor — 点击展开"
    >
      <span
        className="h-2 w-2 rounded-full"
        style={{
          background: "var(--color-accent)",
          boxShadow: "0 0 8px var(--color-accent)",
        }}
      />
    </div>
  );
}
