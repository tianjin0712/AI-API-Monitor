import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CUSTOM_GIF_ID,
  CUSTOM_LUOTIANYI_BACKGROUND_ID,
  LUOTIANYI_BACKGROUNDS,
  LUOTIANYI_GIFS,
  isLuotianyiBackgroundId,
  isLuotianyiGifId,
  luotianyiBackgroundPath,
  luotianyiGifPath,
} from "./themeAssets";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("luotianyiBackgroundPath", () => {
  it("maps built-in ids to their assets", () => {
    expect(luotianyiBackgroundPath("summer-call")).toBe("/themes/luotianyi/background-01.png");
    expect(luotianyiBackgroundPath("summer-holiday")).toBe("/themes/luotianyi/background-10.png");
  });

  it("falls back to the first background for unknown or missing ids", () => {
    expect(luotianyiBackgroundPath("does-not-exist")).toBe(LUOTIANYI_BACKGROUNDS[0][2]);
    expect(luotianyiBackgroundPath(undefined)).toBe(LUOTIANYI_BACKGROUNDS[0][2]);
  });

  it("falls back to the first background when the custom asset is absent", () => {
    vi.stubGlobal("window", { localStorage: { getItem: () => null } });
    expect(luotianyiBackgroundPath(CUSTOM_LUOTIANYI_BACKGROUND_ID)).toBe(LUOTIANYI_BACKGROUNDS[0][2]);
  });

  it("uses the stored custom asset only when it is a safe app-resource URL", () => {
    vi.stubGlobal("window", {
      localStorage: {
        getItem: () => "app-resource://localhost/asset/custom-123",
      },
    });
    expect(luotianyiBackgroundPath(CUSTOM_LUOTIANYI_BACKGROUND_ID)).toBe(
      "app-resource://localhost/asset/custom-123",
    );
  });

  it("ignores unsafe stored values (fail closed)", () => {
    vi.stubGlobal("window", {
      localStorage: {
        getItem: () => "javascript:alert(1)",
      },
    });
    expect(luotianyiBackgroundPath(CUSTOM_LUOTIANYI_BACKGROUND_ID)).toBe(LUOTIANYI_BACKGROUNDS[0][2]);
  });
});

describe("luotianyiGifPath", () => {
  it("maps built-in ids to their assets", () => {
    expect(luotianyiGifPath("idle")).toBe("/themes/luotianyi/idle.gif");
    expect(luotianyiGifPath("heart")).toBe("/themes/luotianyi/heart.gif");
  });

  it("falls back to the first gif for unknown or missing ids", () => {
    expect(luotianyiGifPath("nope")).toBe(LUOTIANYI_GIFS[0][2]);
    expect(luotianyiGifPath(undefined)).toBe(LUOTIANYI_GIFS[0][2]);
  });

  it("uses the stored custom gif only when safe", () => {
    vi.stubGlobal("window", {
      localStorage: { getItem: () => "app-resource://localhost/asset/gif-1" },
    });
    expect(luotianyiGifPath(CUSTOM_GIF_ID)).toBe("app-resource://localhost/asset/gif-1");
  });
});

describe("id guards", () => {
  it("accepts built-in and custom background ids, rejects others", () => {
    expect(isLuotianyiBackgroundId("summer-call")).toBe(true);
    expect(isLuotianyiBackgroundId(CUSTOM_LUOTIANYI_BACKGROUND_ID)).toBe(true);
    expect(isLuotianyiBackgroundId("summer-call-2")).toBe(false);
    expect(isLuotianyiBackgroundId(42)).toBe(false);
    expect(isLuotianyiBackgroundId(undefined)).toBe(false);
  });

  it("accepts built-in and custom gif ids, rejects others", () => {
    expect(isLuotianyiGifId("idle")).toBe(true);
    expect(isLuotianyiGifId(CUSTOM_GIF_ID)).toBe(true);
    expect(isLuotianyiGifId("idle-2")).toBe(false);
    expect(isLuotianyiGifId(null)).toBe(false);
  });
});
