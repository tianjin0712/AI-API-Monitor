import { api } from "../api";
import type { ImportedAsset } from "../types";

export const BACKGROUND_STORAGE_KEY = "ai-monitor-custom-background";
export const BACKGROUND_PALETTE_KEY = "ai-monitor-custom-background-palette";
const BACKGROUND_ASSET_ID_KEY = "ai-monitor-custom-background-asset-id";
export const BACKGROUND_EVENT = "ai-monitor-background-changed";

export type ThemeMode = "dark" | "light";
export type BackgroundPalette = {
  primary: string;
  secondary: string;
  highlight: string;
  mode: ThemeMode;
  /** High-contrast foreground for the compact Mini overlay surface. */
  miniTextColor: string;
  overrides: Record<string, string>;
};
export type NormalizedCrop = { x: number; y: number; width: number; height: number };
type RGB = [number, number, number];

export function clampNormalizedCrop(crop?: NormalizedCrop): NormalizedCrop {
  if (!crop || !Number.isFinite(crop.x) || !Number.isFinite(crop.y)
    || !Number.isFinite(crop.width) || !Number.isFinite(crop.height)
    || crop.width <= 0 || crop.height <= 0) {
    return { x: 0, y: 0, width: 1, height: 1 };
  }
  const x = Math.max(0, Math.min(0.999, crop.x));
  const y = Math.max(0, Math.min(0.999, crop.y));
  return {
    x,
    y,
    width: Math.max(0.001, Math.min(1 - x, crop.width)),
    height: Math.max(0.001, Math.min(1 - y, crop.height)),
  };
}

function isSafeAssetUrl(value: string | null): value is string {
  return !!value && (
    value.startsWith("app-resource://localhost/asset/")
    || value.startsWith("http://app-resource.localhost/asset/")
  );
}

function clampChannel(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function rgbHex([red, green, blue]: RGB): string {
  return `#${[red, green, blue].map((value) => clampChannel(value).toString(16).padStart(2, "0")).join("")}`;
}

function hexRgb(value: string): RGB {
  const normalized = value.replace("#", "");
  return [0, 2, 4].map((offset) => Number.parseInt(normalized.slice(offset, offset + 2), 16)) as RGB;
}

function mix(first: RGB, second: RGB, amount: number): RGB {
  return first.map((value, index) => value + (second[index] - value) * amount) as RGB;
}

function channelLuminance(value: number): number {
  const channel = value / 255;
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

function luminance(color: RGB): number {
  return channelLuminance(color[0]) * 0.2126
    + channelLuminance(color[1]) * 0.7152
    + channelLuminance(color[2]) * 0.0722;
}

export function contrastRatio(first: string, second: string): number {
  const a = luminance(hexRgb(first));
  const b = luminance(hexRgb(second));
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

function colorDistance(first: RGB, second: RGB): number {
  return Math.hypot(first[0] - second[0], first[1] - second[1], first[2] - second[2]) / 441.67;
}

function rgbToHsl([red, green, blue]: RGB): [number, number, number] {
  const r = red / 255, g = green / 255, b = blue / 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const lightness = (max + min) / 2;
  if (max === min) return [0, 0, lightness];
  const delta = max - min;
  const saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min);
  let hue = max === r ? (g - b) / delta + (g < b ? 6 : 0)
    : max === g ? (b - r) / delta + 2
      : (r - g) / delta + 4;
  hue /= 6;
  return [hue, saturation, lightness];
}

function hslToRgb([hue, saturation, lightness]: [number, number, number]): RGB {
  if (saturation === 0) return [lightness * 255, lightness * 255, lightness * 255];
  const hueToRgb = (p: number, q: number, t: number) => {
    let value = t;
    if (value < 0) value += 1;
    if (value > 1) value -= 1;
    if (value < 1 / 6) return p + (q - p) * 6 * value;
    if (value < 1 / 2) return q;
    if (value < 2 / 3) return p + (q - p) * (2 / 3 - value) * 6;
    return p;
  };
  const q = lightness < 0.5
    ? lightness * (1 + saturation)
    : lightness + saturation - lightness * saturation;
  const p = 2 * lightness - q;
  return [hueToRgb(p, q, hue + 1 / 3) * 255, hueToRgb(p, q, hue) * 255, hueToRgb(p, q, hue - 1 / 3) * 255];
}

function readableText(candidate: RGB, background: RGB, minimum: number): RGB {
  let result = candidate;
  const target: RGB = luminance(background) < 0.45 ? [255, 255, 255] : [5, 12, 20];
  for (let index = 0; index < 12 && contrastRatio(rgbHex(result), rgbHex(background)) < minimum; index += 1) {
    result = mix(result, target, 0.18);
  }
  return result;
}

function readableAccent(candidate: RGB, background: RGB): RGB {
  let [hue, saturation, lightness] = rgbToHsl(candidate);
  saturation = Math.max(0.52, saturation);
  const backgroundIsDark = luminance(background) < 0.42;
  lightness = backgroundIsDark ? Math.max(lightness, 0.6) : Math.min(lightness, 0.43);
  let result = hslToRgb([hue, saturation, lightness]);
  for (let index = 0; index < 10 && contrastRatio(rgbHex(result), rgbHex(background)) < 3; index += 1) {
    lightness += backgroundIsDark ? 0.04 : -0.04;
    result = hslToRgb([hue, saturation, Math.max(0.12, Math.min(0.88, lightness))]);
  }
  return result;
}

export function generateThemePalette(colors: RGB[]): BackgroundPalette {
  const samples = colors.length > 0 ? colors : [[40, 60, 80] as RGB];
  const buckets = new Map<string, { color: RGB; count: number }>();
  for (const color of samples) {
    const key = color.map((value) => Math.min(7, Math.floor(value / 32))).join("-");
    const bucket = buckets.get(key);
    if (bucket) {
      bucket.color = bucket.color.map((value, index) => value + color[index]) as RGB;
      bucket.count += 1;
    } else {
      buckets.set(key, { color: [...color] as RGB, count: 1 });
    }
  }
  const clusters = [...buckets.values()]
    .map(({ color, count }) => ({ color: color.map((value) => value / count) as RGB, count }))
    .sort((a, b) => b.count - a.count);
  const primary = clusters[0].color;
  const secondary = clusters.slice(1).sort((a, b) =>
    b.count * (0.2 + colorDistance(primary, b.color)) - a.count * (0.2 + colorDistance(primary, a.color))
  )[0]?.color ?? mix(primary, [255, 255, 255], 0.3);
  const highlight = clusters.slice(1).sort((a, b) => {
    const score = (entry: { color: RGB; count: number }) => {
      const [, saturation] = rgbToHsl(entry.color);
      return saturation * 1.8 + colorDistance(primary, entry.color) + Math.min(0.5, entry.count / samples.length);
    };
    return score(b) - score(a);
  })[0]?.color ?? secondary;
  const averageLuminance = samples.reduce((total, color) => total + luminance(color), 0) / samples.length;
  const mode: ThemeMode = averageLuminance < 0.48 ? "dark" : "light";
  const black: RGB = [5, 12, 20], white: RGB = [250, 253, 255];
  const surface = mode === "dark" ? mix(primary, black, 0.76) : mix(primary, white, 0.86);
  const card = mode === "dark" ? mix(primary, black, 0.62) : mix(primary, white, 0.94);
  const cardHover = mode === "dark" ? mix(primary, black, 0.5) : mix(primary, white, 0.8);
  const control = mode === "dark" ? mix(secondary, black, 0.66) : mix(secondary, white, 0.88);
  const controlHover = mode === "dark" ? mix(secondary, black, 0.55) : mix(secondary, white, 0.76);
  const border = mode === "dark" ? mix(secondary, white, 0.34) : mix(secondary, black, 0.22);
  const accent = readableAccent(highlight, card);
  const accentDim = mix(accent, mode === "dark" ? black : white, 0.22);
  const textPrimary = readableText(mode === "dark" ? white : black, card, 7);
  const textSecondary = readableText(mix(textPrimary, card, 0.25), card, 4.5);
  const textMuted = readableText(mix(textPrimary, card, 0.42), card, 3.2);
  const accentContrast = contrastRatio(rgbHex(accent), "#071019") >= contrastRatio(rgbHex(accent), "#ffffff") ? black : white;
  return {
    primary: rgbHex(primary),
    secondary: rgbHex(secondary),
    highlight: rgbHex(highlight),
    mode,
    miniTextColor: rgbHex(textPrimary),
    overrides: {
      accent: rgbHex(accent),
      "accent-dim": rgbHex(accentDim),
      "accent-contrast": rgbHex(accentContrast),
      surface: rgbHex(surface),
      card: rgbHex(card),
      "card-hover": rgbHex(cardHover),
      control: rgbHex(control),
      "control-hover": rgbHex(controlHover),
      border: rgbHex(border),
      "text-primary": rgbHex(textPrimary),
      "text-secondary": rgbHex(textSecondary),
      "text-muted": rgbHex(textMuted),
      success: mode === "dark" ? "#55d69a" : "#087f5b",
      warning: mode === "dark" ? "#ffd166" : "#a65f00",
      danger: mode === "dark" ? "#ff7f8f" : "#c92a45",
    },
  };
}

function isPalette(value: unknown): value is BackgroundPalette {
  if (!value || typeof value !== "object") return false;
  const palette = value as Partial<BackgroundPalette>;
  return typeof palette.primary === "string" && typeof palette.secondary === "string"
    && typeof palette.highlight === "string" && (palette.mode === "dark" || palette.mode === "light")
    && !!palette.overrides && typeof palette.overrides === "object";
}

function readPalette(): BackgroundPalette | null {
  try {
    const value = JSON.parse(localStorage.getItem(BACKGROUND_PALETTE_KEY) ?? "null") as unknown;
    if (isPalette(value)) return value;
    if (Array.isArray(value) && value.length === 2 && value.every((color) => typeof color === "string")) {
      return generateThemePalette([hexRgb(value[0]), hexRgb(value[1])]);
    }
    return null;
  } catch {
    return null;
  }
}

export function readCustomBackground(): { image: string | null; palette: BackgroundPalette | null } {
  const stored = localStorage.getItem(BACKGROUND_STORAGE_KEY);
  return { image: isSafeAssetUrl(stored) ? stored : null, palette: readPalette() };
}

function analyzeCanvas(canvas: HTMLCanvasElement): BackgroundPalette {
  const sample = document.createElement("canvas");
  sample.width = 32;
  sample.height = 32;
  const context = sample.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("无法分析图片颜色");
  context.drawImage(canvas, 0, 0, sample.width, sample.height);
  const pixels = context.getImageData(0, 0, sample.width, sample.height).data;
  const colors: RGB[] = [];
  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3] < 160) continue;
    colors.push([pixels[index], pixels[index + 1], pixels[index + 2]]);
  }
  return generateThemePalette(colors);
}

export async function analyzeCustomBackground(imageUrl: string): Promise<BackgroundPalette> {
  const assetMatch = imageUrl.match(/\/asset\/([A-Za-z0-9.-]+)$/);
  const blob = assetMatch
    ? new Blob([new Uint8Array(await api.readAsset(assetMatch[1]))])
    : await (await fetch(imageUrl)).blob();
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  const scale = Math.min(1, 640 / Math.max(bitmap.width, bitmap.height));
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("无法分析图片颜色");
  context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  return analyzeCanvas(canvas);
}

export async function prepareCustomBackground(
  file: File,
  crop?: NormalizedCrop,
): Promise<{ bytes: Uint8Array; palette: BackgroundPalette }> {
  const source = await createImageBitmap(file);
  const safeCrop = clampNormalizedCrop(crop);
  const sourceX = safeCrop.x * source.width;
  const sourceY = safeCrop.y * source.height;
  const sourceWidth = safeCrop.width * source.width;
  const sourceHeight = safeCrop.height * source.height;
  const scale = Math.min(1, 1600 / Math.max(sourceWidth, sourceHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(sourceWidth * scale));
  canvas.height = Math.max(1, Math.round(sourceHeight * scale));
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("无法读取图片");
  context.drawImage(source, sourceX, sourceY, sourceWidth, sourceHeight, 0, 0, canvas.width, canvas.height);
  source.close();
  const palette = analyzeCanvas(canvas);
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((value) => value ? resolve(value) : reject(new Error("图片压缩失败")), "image/jpeg", 0.82);
  });
  return { bytes: new Uint8Array(await blob.arrayBuffer()), palette };
}

export function saveCustomBackgroundPalette(palette: BackgroundPalette): void {
  localStorage.setItem(BACKGROUND_PALETTE_KEY, JSON.stringify(palette));
  window.dispatchEvent(new Event(BACKGROUND_EVENT));
}

export function saveCustomBackground(asset: ImportedAsset, palette: BackgroundPalette): void {
  const previousAssetId = localStorage.getItem(BACKGROUND_ASSET_ID_KEY);
  localStorage.setItem(BACKGROUND_STORAGE_KEY, asset.url);
  localStorage.setItem(BACKGROUND_ASSET_ID_KEY, asset.assetId);
  saveCustomBackgroundPalette(palette);
  if (previousAssetId && previousAssetId !== asset.assetId) void api.deleteAsset(previousAssetId).catch(() => {});
}

export async function clearCustomBackground(): Promise<void> {
  const assetId = localStorage.getItem(BACKGROUND_ASSET_ID_KEY);
  localStorage.removeItem(BACKGROUND_STORAGE_KEY);
  localStorage.removeItem(BACKGROUND_ASSET_ID_KEY);
  localStorage.removeItem(BACKGROUND_PALETTE_KEY);
  window.dispatchEvent(new Event(BACKGROUND_EVENT));
  if (assetId) await api.deleteAsset(assetId).catch(() => {});
}

/** One-time migration from the former WebView data-URL storage. */
export async function migrateLegacyBackground(): Promise<void> {
  const legacy = localStorage.getItem(BACKGROUND_STORAGE_KEY);
  if (!legacy?.startsWith("data:image/")) return;
  try {
    const bytes = new Uint8Array(await (await fetch(legacy)).arrayBuffer());
    const asset = await api.importAsset("legacy-background.jpg", bytes);
    const palette = readPalette() ?? generateThemePalette([[16, 32, 48], [24, 56, 77]]);
    saveCustomBackground(asset, palette);
  } catch {
    localStorage.removeItem(BACKGROUND_STORAGE_KEY);
  }
}
