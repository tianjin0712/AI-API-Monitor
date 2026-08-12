import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import TitleBar from "./components/TitleBar";
import MiniBall from "./components/MiniBall";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import type { WindowMode, WindowState } from "./types";

type Page = "dashboard" | "settings";

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [mode, setMode] = useState<WindowMode>("full");

  // 读取窗口状态（模式 + 置顶）
  useEffect(() => {
    void api
      .getWindowState()
      .then((s) => setMode(s.mode))
      .catch(() => {});
  }, []);

  // 监听后端模式/置顶变更事件（托盘菜单等路径统一同步，P0 修复）
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
      <TitleBar onSwitchMode={(m) => void switchMode(m)} />

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
        {page === "dashboard" ? <Dashboard /> : <Settings />}
      </main>
    </div>
  );
}
