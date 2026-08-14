import { describe, expect, it } from "vitest";
import { clampNormalizedCrop, contrastRatio, generateThemePalette } from "./customBackground";

describe("clampNormalizedCrop", () => {
  it("keeps a valid crop unchanged", () => {
    expect(clampNormalizedCrop({ x: 0.1, y: 0.2, width: 0.5, height: 0.6 }))
      .toEqual({ x: 0.1, y: 0.2, width: 0.5, height: 0.6 });
  });

  it("clips a crop to image bounds", () => {
    const crop = clampNormalizedCrop({ x: 0.8, y: 0.9, width: 0.5, height: 0.5 });
    expect(crop.x).toBe(0.8);
    expect(crop.y).toBe(0.9);
    expect(crop.width).toBeCloseTo(0.2);
    expect(crop.height).toBeCloseTo(0.1);
  });

  it("falls back to the whole image for invalid values", () => {
    expect(clampNormalizedCrop({ x: Number.NaN, y: 0, width: 1, height: 1 }))
      .toEqual({ x: 0, y: 0, width: 1, height: 1 });
  });
});

describe("generateThemePalette", () => {
  it("creates a dark palette with readable text for dark images", () => {
    const palette = generateThemePalette([[8, 15, 24], [20, 38, 55], [40, 92, 135], [8, 15, 24]]);
    expect(palette.mode).toBe("dark");
    expect(contrastRatio(palette.overrides["text-primary"], palette.overrides.card)).toBeGreaterThanOrEqual(7);
    expect(contrastRatio(palette.miniTextColor, palette.overrides.card)).toBeGreaterThanOrEqual(7);
    expect(contrastRatio(palette.overrides.accent, palette.overrides.card)).toBeGreaterThanOrEqual(3);
  });

  it("creates a light palette with readable text for light images", () => {
    const palette = generateThemePalette([[245, 247, 250], [224, 236, 246], [95, 170, 220], [245, 247, 250]]);
    expect(palette.mode).toBe("light");
    expect(contrastRatio(palette.overrides["text-primary"], palette.overrides.card)).toBeGreaterThanOrEqual(7);
    expect(contrastRatio(palette.overrides["text-secondary"], palette.overrides.card)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(palette.miniTextColor, palette.overrides.card)).toBeGreaterThanOrEqual(7);
  });
});
