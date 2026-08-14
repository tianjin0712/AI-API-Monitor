import type { Layout } from "../types";

const appliedOverrideKeys = new Set<string>();

/**
 * Applies the complete semantic theme contract to the document root.
 * Components consume semantic CSS tokens only; wallpaper palettes and manual
 * overrides therefore flow through every surface and interaction state.
 */
export function applyThemeTokens(layout: Layout): void {
  const root = document.documentElement;
  root.dataset.theme = layout.theme;
  root.dataset.visualTheme = layout.visualTheme ?? "default";

  const opacity = layout.glassOpacity ?? (layout.theme === "light" ? 0.78 : 0.72);
  const blur = layout.glassBlur ?? 18;
  root.style.setProperty("--glass-opacity", String(opacity));
  root.style.setProperty("--glass-opacity-soft", String(Math.max(0.12, opacity * 0.38)));
  root.style.setProperty("--glass-opacity-light", String(Math.min(0.96, opacity * 0.82)));
  root.style.setProperty("--glass-blur", `${blur}px`);
  if (layout.miniTextColor) root.style.setProperty("--mini-text-color", layout.miniTextColor);
  else root.style.removeProperty("--mini-text-color");
  window.localStorage.setItem("ai-monitor-theme", layout.theme);

  for (const key of appliedOverrideKeys) root.style.removeProperty(`--color-${key}`);
  appliedOverrideKeys.clear();

  if (layout.visualTheme === "custom" || layout.visualTheme === "luotianyi") {
    for (const [key, value] of Object.entries(layout.themeOverrides ?? {})) {
      if (!key || !value) continue;
      root.style.setProperty(`--color-${key}`, value);
      appliedOverrideKeys.add(key);
    }
  }
}
