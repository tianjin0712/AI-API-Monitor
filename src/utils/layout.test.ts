import { describe, expect, it } from "vitest";
import { DEFAULT_WIDGETS, parseTheme, parseWidgets } from "./layout";

describe("layout parsing", () => {
  it("falls back for invalid JSON", () => {
    expect(parseTheme("not-json")).toBe("dark");
    expect(parseWidgets("not-json")).toEqual(DEFAULT_WIDGETS);
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
