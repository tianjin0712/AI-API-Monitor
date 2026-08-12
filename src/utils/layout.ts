import type { DashboardWidget, Layout } from "../types";

/** 默认 Widget 布局（顺序 = 渲染顺序） */
export const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: "w-providers", type: "providers", visible: true },
  { id: "w-summary", type: "summary", visible: true },
  { id: "w-cost", type: "cost", visible: true },
  { id: "w-trend", type: "trend", visible: true },
];

const VALID_TYPES = ["providers", "summary", "cost", "trend"];

/** 解析后端布局 JSON；无效/缺失时回退默认布局（V0.3） */
export function parseWidgets(
  json: string | null,
  defaults: DashboardWidget[] = DEFAULT_WIDGETS,
): DashboardWidget[] {
  let valid: DashboardWidget[] = DEFAULT_WIDGETS;
  if (json) {
    try {
      const parsed = JSON.parse(json) as Partial<Layout>;
      if (Array.isArray(parsed.widgets) && parsed.widgets.length > 0) {
        const filtered = parsed.widgets.filter(
          (w): w is DashboardWidget =>
            !!w &&
            typeof w.id === "string" &&
            VALID_TYPES.includes(w.type) &&
            typeof w.visible === "boolean",
        );
        if (filtered.length > 0) valid = filtered;
      }
    } catch {
      /* 无效布局回退默认 */
    }
  }
  // P2：旧布局自动补入默认清单中新增的 Widget（保留用户顺序与可见性）
  const seen = new Set(valid.map((w) => w.id));
  const merged = [...valid];
  for (const d of defaults) {
    if (!seen.has(d.id)) merged.push(d);
  }
  return merged;
}

/** 解析布局中的主题（无效回退 dark） */
export function parseTheme(json: string | null): "dark" | "light" {
  if (!json) return "dark";
  try {
    const t = (JSON.parse(json) as Partial<Layout>).theme;
    return t === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}
