import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMode } from "../types";

const appWindow = getCurrentWindow();

interface Props {
  /** 切换窗口模式（mini / ball） */
  onSwitchMode: (mode: WindowMode) => void;
}

/** 无边框窗口标题栏：拖拽区 + 模式切换 + 最小化/关闭（透明窗口用） */
export default function TitleBar({ onSwitchMode }: Props) {
  const minimize = () => void appWindow.minimize();
  const close = () => void appWindow.close();

  return (
    <div
      data-tauri-drag-region
      className="flex h-11 shrink-0 items-center justify-between px-4 select-none"
    >
      <div data-tauri-drag-region className="flex items-center gap-2">
        <span
          className="inline-block h-2.5 w-2.5 rounded-full"
          style={{ background: "var(--color-accent)" }}
        />
        <span className="text-[12px] font-semibold tracking-wide text-text-secondary">
          AI API Monitor
        </span>
      </div>

      <div className="flex items-center gap-1">
        {/* Mini 模式 */}
        <button
          aria-label="Mini 窗口"
          title="Mini 窗口"
          onClick={() => onSwitchMode("mini")}
          className="flex h-7 w-7 items-center justify-center rounded-lg text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary"
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <rect x="1.5" y="3" width="9" height="6" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.1" />
            <line x1="1.5" y1="6" x2="10.5" y2="6" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>
        {/* 小球模式 */}
        <button
          aria-label="小球模式"
          title="小球模式"
          onClick={() => onSwitchMode("ball")}
          className="flex h-7 w-7 items-center justify-center rounded-lg text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary"
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <circle cx="6" cy="6" r="4" fill="none" stroke="currentColor" strokeWidth="1.1" />
            <circle cx="6" cy="6" r="1.6" fill="currentColor" />
          </svg>
        </button>
        <span className="mx-0.5 h-4 w-px bg-border" />
        <button
          aria-label="最小化"
          onClick={minimize}
          className="flex h-7 w-7 items-center justify-center rounded-lg text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary"
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <line x1="1.5" y1="6" x2="10.5" y2="6" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
        <button
          aria-label="关闭"
          onClick={close}
          className="flex h-7 w-7 items-center justify-center rounded-lg text-text-secondary transition-colors hover:bg-danger/80 hover:text-white"
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </div>
  );
}
