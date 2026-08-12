import { describe, expect, it } from "vitest";
import { DEFAULT_WIDGETS, parseLayout, parseTheme, parseWidgets } from "./layout";

describe("layout parsing", () => {
  it("falls back for invalid JSON", () => {
    expect(parseTheme("not-json")).toBe("dark");
    expect(parseWidgets("not-json")).toEqual(DEFAULT_WIDGETS);
  });

  it("restores only validated theme overrides", () => {
    const json = JSON.stringify({
      theme: "light",
      widgets: DEFAULT_WIDGETS,
      themeOverrides: { accent: "#123AbC", danger: "red", unknown: "#ffffff" },
    });
    expect(parseLayout(json)).toMatchObject({
      theme: "light",
      themeOverrides: { accent: "#123AbC" },
    });
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
