import { describe, expect, it } from "vitest";
import type { DashboardWidget } from "../types";
import { DEFAULT_WIDGETS, WIDGET_TYPES, nextWidgetId, parseLayout, parseTheme, parseWidgets } from "./layout";

describe("layout parsing", () => {
  it("falls back for invalid JSON", () => {
    expect(parseTheme("not-json")).toBe("dark");
    expect(parseWidgets("not-json")).toEqual(DEFAULT_WIDGETS);
  });

  it("restores only validated theme overrides", () => {
    const json = JSON.stringify({
      theme: "light",
      widgets: DEFAULT_WIDGETS,
      themeOverrides: { accent: "#123AbC", control: "#203040", danger: "red", unknown: "#ffffff" },
    });
    expect(parseLayout(json)).toMatchObject({
      theme: "light",
      themeOverrides: { accent: "#123AbC", control: "#203040" },
    });
  });

  it("restores the optional Luotianyi visual theme", () => {
    const json = JSON.stringify({
      theme: "dark",
      visualTheme: "luotianyi",
      avatarGif: "sing",
      luotianyiBackground: "star-dream",
      widgets: DEFAULT_WIDGETS,
    });
    expect(parseLayout(json).visualTheme).toBe("luotianyi");
    expect(parseLayout(json).avatarGif).toBe("sing");
    expect(parseLayout(json).luotianyiBackground).toBe("star-dream");
  });

  it("restores the custom visual theme", () => {
    const json = JSON.stringify({ theme: "dark", visualTheme: "custom", glassOpacity: 0.63, glassBlur: 24, miniTextColor: "#123456", widgets: DEFAULT_WIDGETS });
    expect(parseLayout(json)).toMatchObject({ visualTheme: "custom", glassOpacity: 0.63, glassBlur: 24, miniTextColor: "#123456" });
  });

  it("clamps wallpaper surface controls", () => {
    const json = JSON.stringify({ theme: "dark", glassOpacity: 2, glassBlur: -5, widgets: DEFAULT_WIDGETS });
    expect(parseLayout(json)).toMatchObject({ glassOpacity: 0.9, glassBlur: 0 });
  });

  it("keeps user order and appends newly introduced widgets", () => {
    const json = JSON.stringify({
      theme: "light",
      widgets: [{ id: "w-trend", type: "trend", visible: false }],
    });
    const widgets = parseWidgets(json);

    expect(widgets[0]).toEqual({
      id: "w-trend",
      type: "trend",
      visible: false,
    });
    expect(widgets).toHaveLength(DEFAULT_WIDGETS.length);
    expect(parseTheme(json)).toBe("light");
  });
});

describe("widget add / remove helpers", () => {
  it("offers the four known widget types", () => {
    expect(WIDGET_TYPES).toEqual(["providers", "summary", "cost", "trend"]);
  });

  it("generates the base id when it is free", () => {
    expect(nextWidgetId(DEFAULT_WIDGETS.slice(0, 1), "trend")).toBe("w-trend");
  });

  it("appends a suffix when the base id is taken", () => {
    const existing: DashboardWidget[] = [
      ...DEFAULT_WIDGETS,
      { id: "w-trend", type: "trend", visible: true },
    ];
    expect(nextWidgetId(existing, "trend")).toBe("w-trend-2");
  });

  it("keeps incrementing until a free id is found", () => {
    const existing: DashboardWidget[] = [
      { id: "w-cost", type: "cost", visible: true },
      { id: "w-cost-2", type: "cost", visible: true },
      { id: "w-cost-3", type: "cost", visible: true },
    ];
    expect(nextWidgetId(existing, "cost")).toBe("w-cost-4");
  });
});
