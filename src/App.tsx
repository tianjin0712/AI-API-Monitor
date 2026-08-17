import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import TitleBar from "./components/TitleBar";
import MiniBall from "./components/MiniBall";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import { DEFAULT_WIDGETS, parseLayout } from "./utils/layout";
import type { DashboardWidget, Layout, WindowMode, WindowState } from "./types";
import { BACKGROUND_EVENT, migrateLegacyBackground, readCustomBackground } from "./utils/customBackground";
import {
  LUOTIANYI_BACKGROUND_EVENT,
  luotianyiBackgroundPath,
  migrateLegacyAvatarGif,
  readCustomLuotianyiBackground,
} from "./utils/themeAssets";
import { applyThemeTokens } from "./theme/applyTheme";
import { MiuixTheme, NavigationBar, Scaffold } from "./components/miuix/Miuix";
import { MonitorStoreProvider } from "./state/MonitorStore";

type Page = "dashboard" | "settings";
export type Theme = "dark" | "light";

const cachedTheme = document.documentElement.dataset.theme;
const DEFAULT_LAYOUT: Layout = {
  theme: cachedTheme === "light" ? "light" : "dark",
  widgets: DEFAULT_WIDGETS,
};

export default function App() {
  return <MonitorStoreProvider><AppShell /></MonitorStoreProvider>;
}

function AppShell() {
  const [page, setPage] = useState<Page>("dashboard");
  const [mode, setMode] = useState<WindowMode>("full");
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  // P1：布局（theme + widgets）作为 App 级单一状态，一次读取、一次写入
  const [layout, setLayout] = useState<Layout>(DEFAULT_LAYOUT);
  const [customBackground, setCustomBackground] = useState(readCustomBackground);
  const [customLuotianyiBackground, setCustomLuotianyiBackground] = useState(readCustomLuotianyiBackground);
  const effectiveBackground = layout.visualTheme === "luotianyi"
    ? (layout.luotianyiBackground === "custom-luotianyi-background"
        ? customLuotianyiBackground ?? luotianyiBackgroundPath()
        : luotianyiBackgroundPath(layout.luotianyiBackground))
    : layout.visualTheme === "custom"
      ? customBackground.image
      : null;
  // 布局加载完成前禁止自动保存，防止默认布局覆盖用户已保存布局（P1 竞态）
  const [layoutLoaded, setLayoutLoaded] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "failed">("idle");
  const saveTimer = useRef<number | undefined>(undefined);
  // 应用主题到 <html data-theme>（V0.3）+ 自定义色值覆盖（V1.0 主题分享）
  useEffect(() => {
    applyThemeTokens(layout);
  }, [layout]);

  useEffect(() => {
    void Promise.all([migrateLegacyBackground(), migrateLegacyAvatarGif()]).then(() => {
      setCustomBackground(readCustomBackground());
      window.dispatchEvent(new Event(BACKGROUND_EVENT));
    });
    const update = () => setCustomBackground(readCustomBackground());
    const updateLuotianyi = () => setCustomLuotianyiBackground(readCustomLuotianyiBackground());
    window.addEventListener(BACKGROUND_EVENT, update);
    window.addEventListener(LUOTIANYI_BACKGROUND_EVENT, updateLuotianyi);
    window.addEventListener("storage", update);
    window.addEventListener("storage", updateLuotianyi);
    return () => {
      window.removeEventListener(BACKGROUND_EVENT, update);
      window.removeEventListener(LUOTIANYI_BACKGROUND_EVENT, updateLuotianyi);
      window.removeEventListener("storage", update);
      window.removeEventListener("storage", updateLuotianyi);
    };
  }, []);

  useEffect(() => {
    const el = document.documentElement;
    el.classList.toggle("has-user-background", !!effectiveBackground);
    if (layout.visualTheme === "custom" && customBackground.palette) {
      el.style.setProperty("--user-bg-color-1", customBackground.palette.primary);
      el.style.setProperty("--user-bg-color-2", customBackground.palette.secondary);
    } else {
      el.style.removeProperty("--user-bg-color-1");
      el.style.removeProperty("--user-bg-color-2");
    }
    if (effectiveBackground) el.style.setProperty("--custom-background-image", `url("${effectiveBackground}")`);
    else el.style.removeProperty("--custom-background-image");
  }, [customBackground, effectiveBackground, layout.visualTheme]);

  // 启动：读取窗口状态 + 布局（theme/widgets）
  useEffect(() => {
    void api
      .getWindowState()
      .then((s) => { setMode(s.mode); setAlwaysOnTop(s.alwaysOnTop); })
      .catch(() => {});
    void api
      .getLayout()
      .then((json) => {
        setLayout(parseLayout(json));
      })
      .catch(() => {})
      .finally(() => setLayoutLoaded(true));
  }, []);

  useEffect(() => {
    void api.getDatabaseRecoveryNotice().then((notice) => {
      if (notice) window.alert(notice);
    }).catch(() => {});
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
    return <MiniBall mode={mode} visualTheme={layout.visualTheme} avatarGif={layout.avatarGif} backgroundImage={effectiveBackground} floatingScrollMode={layout.floatingScrollMode} onSwitchMode={(next) => void switchMode(next)} onExpand={() => void switchMode("full")} />;
  }

  // ---- Mini 模式 ----
  if (mode === "mini") {
    return (
      <MiniBall
        mode={mode}
        visualTheme={layout.visualTheme}
        avatarGif={layout.avatarGif}
        backgroundImage={effectiveBackground}
        floatingScrollMode={layout.floatingScrollMode}
        onSwitchMode={(next) => void switchMode(next)}
        onExpand={() => void switchMode("full")}
        compact
      />
    );
  }

  // ---- Full 模式 ----
  return (
    <MiuixTheme>
    <div className={`app-shell h-screen overflow-hidden border ${effectiveBackground ? "custom-background-shell" : ""}`} style={effectiveBackground ? { background: "transparent" } : undefined}>
    <Scaffold
      topBar={<TitleBar
        onSwitchMode={(m) => void switchMode(m)}
        theme={layout.theme}
        onToggleTheme={toggleTheme}
        alwaysOnTop={alwaysOnTop}
        onToggleAlwaysOnTop={() => {
          const next = !alwaysOnTop;
          setAlwaysOnTop(next);
          void api.setAlwaysOnTop(next).catch(() => setAlwaysOnTop(!next));
        }}
      />}
      navigationBar={<NavigationBar
        selected={page}
        onSelect={(id) => setPage(id as Page)}
        items={[
          { id: "dashboard", label: "总览", icon: <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8.5h4V14H2zm8-6h4V14h-4zM2 2h4v3H2z" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" /></svg> },
          { id: "settings", label: "设置", icon: <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 5.7A2.3 2.3 0 1 0 8 10.3 2.3 2.3 0 0 0 8 5.7Zm0-3.9.8 1.4 1.6.4 1.4-.7 1.3 1.3-.7 1.4.4 1.6 1.4.8v1.8l-1.4.8-.4 1.6.7 1.4-1.3 1.3-1.4-.7-1.6.4-.8 1.4H7.2l-.8-1.4-1.6-.4-1.4.7-1.3-1.3.7-1.4-.4-1.6L1 8.9V7.1l1.4-.8.4-1.6-.7-1.4L3.4 2l1.4.7 1.6-.4.8-1.4Z" fill="none" stroke="currentColor" strokeWidth="1.15" strokeLinejoin="round" /></svg> },
        ]}
      />}
    >

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

      <main className="app-content min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
        {page === "dashboard" ? (
          <Dashboard
            widgets={layout.widgets}
            visualTheme={layout.visualTheme}
            avatarGif={layout.avatarGif}
            onWidgetsChange={updateWidgets}
          />
        ) : (
          <Settings layout={layout} onLayoutChange={updateLayout} />
        )}
      </main>
    </Scaffold>
    </div>
    </MiuixTheme>
  );
}
