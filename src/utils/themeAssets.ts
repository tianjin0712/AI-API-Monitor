import { api } from "../api";
import type { ImportedAsset } from "../types";

export const LUOTIANYI_GIFS = [
  ["idle", "闲置", "/themes/luotianyi/idle.gif"],
  ["working", "工作", "/themes/luotianyi/working.gif"],
  ["loading", "加载中", "/themes/luotianyi/loading.gif"],
  ["success", "点赞", "/themes/luotianyi/success.gif"],
  ["arrive", "到达", "/themes/luotianyi/arrive.gif"],
  ["cry", "哭泣", "/themes/luotianyi/cry.gif"],
  ["watermelon", "吃西瓜", "/themes/luotianyi/watermelon.gif"],
  ["shy", "害羞", "/themes/luotianyi/shy.gif"],
  ["rose", "玫瑰", "/themes/luotianyi/rose.gif"],
  ["dazed", "发呆", "/themes/luotianyi/dazed.gif"],
  ["sing", "唱歌", "/themes/luotianyi/sing.gif"],
  ["megaphone", "扩音器", "/themes/luotianyi/megaphone.gif"],
  ["gift", "礼物", "/themes/luotianyi/gift.gif"],
  ["angry", "生气打字", "/themes/luotianyi/angry.gif"],
  ["mute", "静音", "/themes/luotianyi/mute.gif"],
  ["heart", "爱心", "/themes/luotianyi/heart.gif"],
] as const;

export const LUOTIANYI_BACKGROUNDS = [
  ["summer-call", "夏日电话", "/themes/luotianyi/background-01.png"],
  ["blue-stage", "蓝色舞台", "/themes/luotianyi/background-02.png"],
  ["sunset-field", "夕阳花田", "/themes/luotianyi/background-03.png"],
  ["record-party", "唱片派对", "/themes/luotianyi/background-04.png"],
  ["night-city", "夜色都市", "/themes/luotianyi/background-05.png"],
  ["balloon-day", "气球晴日", "/themes/luotianyi/background-06.png"],
  ["red-night", "红夜舞会", "/themes/luotianyi/background-07.png"],
  ["star-dream", "星河梦境", "/themes/luotianyi/background-08.png"],
  ["rainy-day", "雨日之歌", "/themes/luotianyi/background-09.png"],
  ["summer-holiday", "盛夏假日", "/themes/luotianyi/background-10.png"],
] as const;

export type LuotianyiBackgroundId = (typeof LUOTIANYI_BACKGROUNDS)[number][0];
export const CUSTOM_LUOTIANYI_BACKGROUND_ID = "custom-luotianyi-background";
export const LUOTIANYI_BACKGROUND_EVENT = "ai-monitor-luotianyi-background-changed";
const CUSTOM_LUOTIANYI_BACKGROUND_STORAGE_KEY = "ai-monitor-custom-luotianyi-background";
const CUSTOM_LUOTIANYI_BACKGROUND_ASSET_ID_KEY = "ai-monitor-custom-luotianyi-background-asset-id";

export type LuotianyiGifId = (typeof LUOTIANYI_GIFS)[number][0];
export const CUSTOM_GIF_ID = "custom-gif";
const CUSTOM_GIF_STORAGE_KEY = "ai-monitor-custom-avatar-gif";
const CUSTOM_GIF_ASSET_ID_KEY = "ai-monitor-custom-avatar-gif-asset-id";

function isSafeAssetUrl(value: string | null): value is string {
  return !!value && (
    value.startsWith("app-resource://localhost/asset/")
    || value.startsWith("http://app-resource.localhost/asset/")
  );
}

export function readCustomAvatarGif(): string | null {
  const value = window.localStorage.getItem(CUSTOM_GIF_STORAGE_KEY);
  return isSafeAssetUrl(value) ? value : null;
}

export function readCustomLuotianyiBackground(): string | null {
  const value = window.localStorage.getItem(CUSTOM_LUOTIANYI_BACKGROUND_STORAGE_KEY);
  return isSafeAssetUrl(value) ? value : null;
}

export function saveCustomLuotianyiBackground(asset: ImportedAsset): void {
  const previousAssetId = window.localStorage.getItem(CUSTOM_LUOTIANYI_BACKGROUND_ASSET_ID_KEY);
  window.localStorage.setItem(CUSTOM_LUOTIANYI_BACKGROUND_STORAGE_KEY, asset.url);
  window.localStorage.setItem(CUSTOM_LUOTIANYI_BACKGROUND_ASSET_ID_KEY, asset.assetId);
  window.dispatchEvent(new Event(LUOTIANYI_BACKGROUND_EVENT));
  if (previousAssetId && previousAssetId !== asset.assetId) {
    void api.deleteAsset(previousAssetId).catch(() => {});
  }
}

export function luotianyiBackgroundPath(id?: string): string {
  if (id === CUSTOM_LUOTIANYI_BACKGROUND_ID) {
    return readCustomLuotianyiBackground() ?? LUOTIANYI_BACKGROUNDS[0][2];
  }
  return LUOTIANYI_BACKGROUNDS.find(([key]) => key === id)?.[2] ?? LUOTIANYI_BACKGROUNDS[0][2];
}

export function isLuotianyiBackgroundId(value: unknown): value is LuotianyiBackgroundId | typeof CUSTOM_LUOTIANYI_BACKGROUND_ID {
  return typeof value === "string" && (
    value === CUSTOM_LUOTIANYI_BACKGROUND_ID
    || LUOTIANYI_BACKGROUNDS.some(([key]) => key === value)
  );
}

export function saveCustomAvatarGif(asset: ImportedAsset): void {
  const previousAssetId = window.localStorage.getItem(CUSTOM_GIF_ASSET_ID_KEY);
  window.localStorage.setItem(CUSTOM_GIF_STORAGE_KEY, asset.url);
  window.localStorage.setItem(CUSTOM_GIF_ASSET_ID_KEY, asset.assetId);
  if (previousAssetId && previousAssetId !== asset.assetId) {
    void api.deleteAsset(previousAssetId).catch(() => {});
  }
}

export async function migrateLegacyAvatarGif(): Promise<void> {
  const legacy = window.localStorage.getItem(CUSTOM_GIF_STORAGE_KEY);
  if (!legacy?.startsWith("data:image/gif")) return;
  try {
    const bytes = new Uint8Array(await (await fetch(legacy)).arrayBuffer());
    saveCustomAvatarGif(await api.importAsset("legacy-avatar.gif", bytes));
  } catch {
    window.localStorage.removeItem(CUSTOM_GIF_STORAGE_KEY);
  }
}

export function luotianyiGifPath(id?: string): string {
  if (id === CUSTOM_GIF_ID) return readCustomAvatarGif() ?? LUOTIANYI_GIFS[0][2];
  return LUOTIANYI_GIFS.find(([key]) => key === id)?.[2] ?? LUOTIANYI_GIFS[0][2];
}

export function isLuotianyiGifId(value: unknown): value is LuotianyiGifId {
  return typeof value === "string" && (value === CUSTOM_GIF_ID || LUOTIANYI_GIFS.some(([key]) => key === value));
}
