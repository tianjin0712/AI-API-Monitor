import type { DashboardWidget, Layout, WidgetType } from "../types";
import { isLuotianyiBackgroundId, isLuotianyiGifId } from "./themeAssets";

export const THEME_OVERRIDE_KEYS = [
  "accent", "accent-dim", "accent-contrast", "surface", "card", "card-hover",
  "control", "control-hover", "border", "text-primary", "text-secondary", "text-muted",
  "success", "warning", "danger",
] as const;
const THEME_OVERRIDE_KEY_SET = new Set<string>(THEME_OVERRIDE_KEYS);
const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;

/** 默认 Widget 布局（顺序 = 渲染顺序） */
export const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: "w-providers", type: "providers", visible: true },
  { id: "w-summary", type: "summary", visible: true },
  { id: "w-cost", type: "cost", visible: true },
  { id: "w-trend", type: "trend", visible: true },
];

const VALID_TYPES = ["providers", "summary", "cost", "trend"] as const;

/** 可用的 Widget 类型（编辑模式下可添加/删除，每类最多一个实例）。 */
export const WIDGET_TYPES: WidgetType[] = [...VALID_TYPES];

/** Widget 类型的中文名（编辑模式添加菜单）。 */
export const WIDGET_TYPE_LABELS: Record<WidgetType, string> = {
  providers: "账户列表",
  summary: "今日汇总",
  cost: "费用概览",
  trend: "趋势",
};

/** 生成与现有 id 不冲突的 Widget id（`w-<type>`，冲突时追加序号）。 */
export function nextWidgetId(existing: DashboardWidget[], type: WidgetType): string {
  const used = new Set(existing.map((widget) => widget.id));
  let candidate = `w-${type}`;
  let suffix = 2;
  while (used.has(candidate)) {
    candidate = `w-${type}-${suffix}`;
    suffix += 1;
  }
  return candidate;
}

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

/** Parse the complete persisted layout, including validated V1.0 theme overrides. */
export function parseLayout(json: string | null): Layout {
  const layout: Layout = {
    theme: parseTheme(json),
    widgets: parseWidgets(json),
  };
  if (!json) return layout;
  try {
    const parsed = JSON.parse(json) as Partial<Layout>;
    if (parsed.visualTheme === "luotianyi" || parsed.visualTheme === "custom") layout.visualTheme = parsed.visualTheme;
    if (isLuotianyiGifId(parsed.avatarGif)) layout.avatarGif = parsed.avatarGif;
    if (isLuotianyiBackgroundId(parsed.luotianyiBackground)) layout.luotianyiBackground = parsed.luotianyiBackground;
    if (typeof parsed.glassOpacity === "number" && Number.isFinite(parsed.glassOpacity)) {
      layout.glassOpacity = Math.max(0.15, Math.min(0.9, parsed.glassOpacity));
    }
    if (typeof parsed.glassBlur === "number" && Number.isFinite(parsed.glassBlur)) {
      layout.glassBlur = Math.max(0, Math.min(32, parsed.glassBlur));
    }
    if (typeof parsed.miniTextColor === "string" && HEX_COLOR.test(parsed.miniTextColor)) {
      layout.miniTextColor = parsed.miniTextColor;
    }
    if (parsed.floatingScrollMode === "auto" || parsed.floatingScrollMode === "wheel") {
      layout.floatingScrollMode = parsed.floatingScrollMode;
    }
    if (!parsed.themeOverrides || typeof parsed.themeOverrides !== "object") return layout;
    const overrides: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed.themeOverrides)) {
      if (THEME_OVERRIDE_KEY_SET.has(key) && typeof value === "string" && HEX_COLOR.test(value)) {
        overrides[key] = value;
      }
    }
    if (Object.keys(overrides).length > 0) layout.themeOverrides = overrides;
  } catch {
    // theme/widgets parsers already provide safe defaults.
  }
  return layout;
}
