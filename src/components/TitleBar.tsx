import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

/** 无边框窗口标题栏：拖拽区 + 最小化/关闭（透明窗口用） */
export default function TitleBar() {
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
