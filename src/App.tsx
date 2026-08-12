import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import TitleBar from "./components/TitleBar";
import MiniBall from "./components/MiniBall";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import { DEFAULT_WIDGETS, parseTheme, parseWidgets } from "./utils/layout";
import type { DashboardWidget, Layout, WindowMode, WindowState } from "./types";

type Page = "dashboard" | "settings";
export type Theme = "dark" | "light";

const DEFAULT_LAYOUT: Layout = { theme: "dark", widgets: DEFAULT_WIDGETS };

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [mode, setMode] = useState<WindowMode>("full");
  // P1：布局（theme + widgets）作为 App 级单一状态，一次读取、一次写入
  const [layout, setLayout] = useState<Layout>(DEFAULT_LAYOUT);
  // 布局加载完成前禁止自动保存，防止默认布局覆盖用户已保存布局（P1 竞态）
  const [layoutLoaded, setLayoutLoaded] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "failed">("idle");
  const saveTimer = useRef<number | undefined>(undefined);

  // 应用主题到 <html data-theme>
  useEffect(() => {
    document.documentElement.dataset.theme = layout.theme;
  }, [layout.theme]);

  // 启动：读取窗口状态 + 布局（theme/widgets）
  useEffect(() => {
    void api
      .getWindowState()
      .then((s) => setMode(s.mode))
      .catch(() => {});
    void api
      .getLayout()
      .then((json) => {
        setLayout({ theme: parseTheme(json), widgets: parseWidgets(json) });
      })
      .catch(() => {})
      .finally(() => setLayoutLoaded(true));
  }, []);

  // 布局持久化（P1：统一保存入口，防抖 + 失败可见）
  const persistLayout = useCallback((next: Layout) => {
    window.clearTimeout(saveTimer.current);
    setSaveState("saving");
    saveTimer.current = window.setTimeout(() => {
      void api
        .setLayout(JSON.stringify(next))
        .then(() => setSaveState("idle"))
        .catch(() => setSaveState("failed"));
    }, 500);
  }, []);

  const updateLayout = useCallback(
    (updater: (prev: Layout) => Layout) => {
      setLayout((prev) => {
        const next = updater(prev);
        if (layoutLoaded) persistLayout(next);
        return next;
      });
    },
    [layoutLoaded, persistLayout],
  );

  // Widgets 变更（Dashboard 编辑/拖拽/可见性）
  const updateWidgets = useCallback(
    (updater: (ws: DashboardWidget[]) => DashboardWidget[]) => {
      updateLayout((prev) => ({ ...prev, widgets: updater(prev.widgets) }));
    },
    [updateLayout],
  );

  // 主题切换（标题栏全局，设置页/Dashboard 均生效并持久化）
  const toggleTheme = useCallback(() => {
    updateLayout((prev) => ({
      ...prev,
      theme: prev.theme === "dark" ? "light" : "dark",
    }));
  }, [updateLayout]);

  // 监听后端模式/置顶变更事件（托盘菜单等路径统一同步）
  useEffect(() => {
    const unlisten = listen<WindowState>("window-mode-changed", (e) => {
      setMode(e.payload.mode);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const switchMode = async (m: WindowMode) => {
    try {
      const s = await api.setWindowMode(m);
      setMode(s.mode);
    } catch (e) {
      console.error("切换窗口模式失败:", e);
    }
  };

  // ---- 小球模式 ----
  if (mode === "ball") {
    return <MiniBall mode={mode} onExpand={() => void switchMode("full")} />;
  }

  // ---- Mini 模式 ----
  if (mode === "mini") {
    return (
      <MiniBall
        mode={mode}
        onExpand={() => void switchMode("full")}
        compact
      />
    );
  }

  // ---- Full 模式 ----
  return (
    <div
      className="flex h-screen flex-col overflow-hidden"
      style={{
        background:
          "radial-gradient(120% 90% at 50% 0%, rgba(108,140,255,0.10), transparent 60%), var(--color-surface)",
      }}
    >
      <TitleBar
        onSwitchMode={(m) => void switchMode(m)}
        theme={layout.theme}
        onToggleTheme={toggleTheme}
      />

      {/* 布局保存状态（P1：失败可见，不静默） */}
      {saveState === "failed" && (
        <div className="mx-4 mb-1 flex items-center justify-between rounded-lg border border-danger/30 bg-danger/10 px-3 py-1.5 text-[11px] text-danger">
          <span>布局保存失败，调整可能未生效</span>
          <button
            className="underline underline-offset-2"
            onClick={() => persistLayout(layout)}
          >
            重试
          </button>
        </div>
      )}

      {/* 页面切换 */}
      <nav className="mx-4 flex shrink-0 gap-1 rounded-xl border border-border/60 bg-white/[0.03] p-1">
        {(
          [
            ["dashboard", "总览"],
            ["settings", "设置"],
          ] as [Page, string][]
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setPage(key)}
            className={`flex-1 rounded-lg py-1.5 text-[13px] font-medium transition-colors ${
              page === key
                ? "bg-accent text-[#0b0e14]"
                : "text-text-secondary hover:bg-white/5 hover:text-text-primary"
            }`}
          >
            {label}
          </button>
        ))}
      </nav>

      <main className="min-h-0 flex-1 overflow-y-auto px-4 pb-4 pt-3">
        {page === "dashboard" ? (
          <Dashboard
            widgets={layout.widgets}
            onWidgetsChange={updateWidgets}
          />
        ) : (
          <Settings />
        )}
      </main>
    </div>
  );
}
