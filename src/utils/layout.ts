import type { DashboardWidget, Layout } from "../types";

/** 默认 Widget 布局（顺序 = 渲染顺序） */
export const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: "w-providers", type: "providers", visible: true },
  { id: "w-summary", type: "summary", visible: true },
  { id: "w-cost", type: "cost", visible: true },
];

const VALID_TYPES = ["providers", "summary", "cost"];

/** 解析后端布局 JSON；无效/缺失时回退默认布局（V0.3） */
export function parseWidgets(json: string | null): DashboardWidget[] {
  if (!json) return DEFAULT_WIDGETS;
  try {
    const parsed = JSON.parse(json) as Partial<Layout>;
    if (!Array.isArray(parsed.widgets) || parsed.widgets.length === 0) {
      return DEFAULT_WIDGETS;
    }
    const valid = parsed.widgets.filter(
      (w): w is DashboardWidget =>
        !!w &&
        typeof w.id === "string" &&
        VALID_TYPES.includes(w.type) &&
        typeof w.visible === "boolean",
    );
    return valid.length > 0 ? valid : DEFAULT_WIDGETS;
  } catch {
    return DEFAULT_WIDGETS;
  }
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
