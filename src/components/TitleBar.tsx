import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMode } from "../types";
import type { Theme } from "../App";

const appWindow = getCurrentWindow();

interface Props {
  /** 切换窗口模式（mini / ball） */
  onSwitchMode: (mode: WindowMode) => void;
  /** 当前主题（V0.3） */
  theme: Theme;
  onToggleTheme: () => void;
}

/** 无边框窗口标题栏：拖拽区 + 模式切换 + 主题 + 最小化/关闭（透明窗口用） */
export default function TitleBar({ onSwitchMode, theme, onToggleTheme }: Props) {
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
        {/* 主题切换（V0.3） */}
        <button
          aria-label="切换主题"
          title={theme === "dark" ? "切换到亮色主题" : "切换到暗色主题"}
          onClick={onToggleTheme}
          className="flex h-7 w-7 items-center justify-center rounded-lg text-text-secondary transition-colors hover:bg-white/10 hover:text-text-primary"
        >
          {theme === "dark" ? (
            <svg width="12" height="12" viewBox="0 0 12 12">
              <circle cx="6" cy="6" r="3.2" fill="none" stroke="currentColor" strokeWidth="1.1" />
              <g stroke="currentColor" strokeWidth="1">
                <line x1="6" y1="1" x2="6" y2="2.2" />
                <line x1="6" y1="9.8" x2="6" y2="11" />
                <line x1="1" y1="6" x2="2.2" y2="6" />
                <line x1="9.8" y1="6" x2="11" y2="6" />
                <line x1="2.5" y1="2.5" x2="3.3" y2="3.3" />
                <line x1="8.7" y1="8.7" x2="9.5" y2="9.5" />
                <line x1="2.5" y1="9.5" x2="3.3" y2="8.7" />
                <line x1="8.7" y1="3.3" x2="9.5" y2="2.5" />
              </g>
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 12 12">
              <path
                d="M10.2 7.6A4.6 4.6 0 0 1 4.4 1.8a4.6 4.6 0 1 0 5.8 5.8Z"
                fill="currentColor"
              />
            </svg>
          )}
        </button>
        <span className="mx-0.5 h-4 w-px bg-border" />
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
