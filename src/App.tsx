import { useState } from "react";
import TitleBar from "./components/TitleBar";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";

type Page = "dashboard" | "settings";

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");

  return (
    <div
      className="flex h-screen flex-col overflow-hidden"
      style={{
        background:
          "radial-gradient(120% 90% at 50% 0%, rgba(108,140,255,0.10), transparent 60%), var(--color-surface)",
      }}
    >
      <TitleBar />

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
