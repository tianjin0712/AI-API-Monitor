import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMode } from "../types";
import type { Theme } from "../App";
import type { ButtonHTMLAttributes, ReactNode } from "react";

function withAppWindow(action: (window: ReturnType<typeof getCurrentWindow>) => Promise<void>, errorLabel: string) {
  try {
    void action(getCurrentWindow()).catch((error) => console.error(`${errorLabel}:`, error));
  } catch (error) {
    console.info(`${errorLabel}（浏览器预览模式）:`, error);
  }
}

interface Props {
  /** 切换窗口模式（mini / ball） */
  onSwitchMode: (mode: WindowMode) => void;
  /** 当前主题（V0.3） */
  theme: Theme;
  onToggleTheme: () => void;
  alwaysOnTop: boolean;
  onToggleAlwaysOnTop: () => void;
}

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  selected?: boolean;
  danger?: boolean;
}

/** Shared, non-dragging icon control for every title-bar action. */
function IconButton({ children, className = "", selected = false, danger = false, ...props }: IconButtonProps) {
  return (
    <button
      type="button"
      className={`title-icon-button ${selected ? "is-active" : ""} ${danger ? "is-danger" : ""} ${className}`}
      {...props}
    >
      <span className="title-icon-button-content">{children}</span>
    </button>
  );
}

/** 无边框窗口标题栏：拖拽区 + 模式切换 + 主题 + 最小化/关闭（透明窗口用） */
export default function TitleBar({ onSwitchMode, theme, onToggleTheme, alwaysOnTop, onToggleAlwaysOnTop }: Props) {
  const minimize = () => {
    withAppWindow((window) => window.minimize(), "最小化窗口失败");
  };
  const close = () => {
    withAppWindow((window) => window.close(), "关闭窗口失败");
  };

  return (
    <div
      data-tauri-drag-region
      className="title-toolbar mx-2 mt-1 flex h-9 shrink-0 items-center justify-between px-2.5 select-none"
    >
      <div data-tauri-drag-region className="flex items-center gap-2">
        <IconButton aria-label={alwaysOnTop ? "取消置顶" : "窗口置顶"} title={alwaysOnTop ? "取消置顶" : "窗口置顶"} selected={alwaysOnTop} onClick={onToggleAlwaysOnTop}>
          <svg className="title-pin-icon" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M4 1.5h4l-.6 3 1.7 1.7v.7H6.6V11H5.4V6.9H2.9v-.7l1.7-1.7L4 1.5Z" fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" /></svg>
        </IconButton>
        <span className="title-brand-mark" aria-hidden="true">
          <svg width="14" height="14" viewBox="0 0 14 14"><path d="M2 9.5V6.8M5.3 11V3M8.7 9.3V5M12 11.5V2" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" /></svg>
        </span>
        <span className="text-[12px] font-semibold tracking-wide text-text-secondary">
          AI API Monitor
        </span>
      </div>

      <div className="title-toolbar-actions flex items-center">
        {/* 主题切换（V0.3） */}
        <IconButton
          aria-label="切换主题"
          title={theme === "dark" ? "切换到亮色主题" : "切换到暗色主题"}
          onClick={onToggleTheme}
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
        </IconButton>
        {/* Mini 模式 */}
        <IconButton
          aria-label="Mini 窗口"
          title="Mini 窗口"
          onClick={() => onSwitchMode("mini")}
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <rect x="1.5" y="3" width="9" height="6" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.1" />
            <line x1="1.5" y1="6" x2="10.5" y2="6" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </IconButton>
        {/* 小球模式 */}
        <IconButton
          aria-label="小球模式"
          title="小球模式"
          onClick={() => onSwitchMode("ball")}
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <circle cx="6" cy="6" r="4" fill="none" stroke="currentColor" strokeWidth="1.1" />
            <circle cx="6" cy="6" r="1.6" fill="currentColor" />
          </svg>
        </IconButton>
        <IconButton
          aria-label="最小化"
          onClick={minimize}
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <line x1="1.5" y1="6" x2="10.5" y2="6" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </IconButton>
        <IconButton
          aria-label="关闭"
          onClick={close}
          danger
        >
          <svg width="12" height="12" viewBox="0 0 12 12">
            <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </IconButton>
      </div>
    </div>
  );
}
