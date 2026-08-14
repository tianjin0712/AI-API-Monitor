import { describe, expect, it } from "vitest";
import type { ProviderUsage, RefreshResult, RefreshSettings } from "../types";
import {
  anyRefreshFailed,
  computeRefreshIntervalSecs,
  mergeErrorResults,
  mergeUsageResults,
  nextRefreshState,
  refreshSucceeded,
} from "./refreshLogic";

const settings: RefreshSettings = { foregroundSecs: 10, backgroundSecs: 60 };

function usage(id: number, tokens: number): ProviderUsage {
  return {
    providerId: id,
    provider: `p${id}`,
    balance: null,
    currency: "",
    totalTokens: tokens,
    inputTokens: 0,
    outputTokens: 0,
    cachedTokens: 0,
    todayTokens: null,
    todayCost: null,
    monthCost: null,
    remaining: null,
    resetTime: null,
    codex: null,
    updatedAt: "2026-08-14T00:00:00Z",
  };
}

function result(
  providerId: number,
  success: boolean,
  overrides: Partial<RefreshResult> = {},
): RefreshResult {
  return {
    providerId,
    provider: `p${providerId}`,
    success,
    usage: success ? usage(providerId, providerId * 100) : null,
    error: success ? null : `err-${providerId}`,
    ...overrides,
  };
}

describe("computeRefreshIntervalSecs", () => {
  it("clamps foreground below 10s to 10s", () => {
    expect(computeRefreshIntervalSecs({ ...settings, foregroundSecs: 5 }, true)).toBe(10);
    expect(computeRefreshIntervalSecs({ ...settings, foregroundSecs: 0 }, true)).toBe(10);
  });

  it("clamps background below 60s to 60s", () => {
    expect(computeRefreshIntervalSecs({ ...settings, backgroundSecs: 30 }, false)).toBe(60);
    expect(computeRefreshIntervalSecs({ ...settings, backgroundSecs: 0 }, false)).toBe(60);
  });

  it("keeps configured values above the minimums", () => {
    expect(computeRefreshIntervalSecs({ ...settings, foregroundSecs: 30 }, true)).toBe(30);
    expect(computeRefreshIntervalSecs({ ...settings, backgroundSecs: 120 }, false)).toBe(120);
  });

  it("switches between foreground and background by visibility", () => {
    expect(computeRefreshIntervalSecs(settings, true)).toBe(10);
    expect(computeRefreshIntervalSecs(settings, false)).toBe(60);
  });
});

describe("mergeUsageResults", () => {
  it("adds successful usages with a providerId", () => {
    const next = mergeUsageResults({}, [result(1, true), result(2, true)]);
    expect(Object.keys(next).sort()).toEqual(["1", "2"]);
    expect(next[1].totalTokens).toBe(100);
  });

  it("skips successful results without a providerId", () => {
    const next = mergeUsageResults({}, [result(1, true, { usage: { ...usage(1, 5), providerId: null } })]);
    expect(Object.keys(next)).toEqual([]);
  });

  it("keeps previous usages for failed providers", () => {
    const before = { 1: usage(1, 100) };
    const next = mergeUsageResults(before, [result(1, false)]);
    expect(next[1].totalTokens).toBe(100);
  });

  it("overwrites an existing usage with a fresh successful one", () => {
    const before = { 1: usage(1, 100) };
    const next = mergeUsageResults(before, [result(1, true, { usage: usage(1, 250) })]);
    expect(next[1].totalTokens).toBe(250);
  });
});

describe("mergeErrorResults", () => {
  it("clears the error for successful providers", () => {
    const before = { 1: "old error" };
    const next = mergeErrorResults(before, [result(1, true)]);
    expect(1 in next).toBe(false);
  });

  it("records the error for failed providers", () => {
    const next = mergeErrorResults({}, [result(1, false)]);
    expect(next[1]).toBe("err-1");
  });

  it("uses a default message when the error is missing", () => {
    const next = mergeErrorResults({}, [result(1, false, { error: null })]);
    expect(next[1]).toBe("刷新失败");
  });

  it("does not touch unrelated providers", () => {
    const before = { 9: "unrelated" };
    const next = mergeErrorResults(before, [result(1, true)]);
    expect(next[9]).toBe("unrelated");
  });
});

describe("refreshSucceeded / anyRefreshFailed / nextRefreshState", () => {
  it("succeeds when at least one provider returns usage", () => {
    expect(refreshSucceeded([result(1, true), result(2, false)])).toBe(true);
  });

  it("does not count a success without usage data", () => {
    expect(refreshSucceeded([result(1, true, { usage: null })])).toBe(false);
  });

  it("detects any failure", () => {
    expect(anyRefreshFailed([result(1, true), result(2, false)])).toBe(true);
    expect(anyRefreshFailed([result(1, true)])).toBe(false);
  });

  it("is success when at least one provider has data", () => {
    expect(nextRefreshState([result(1, true), result(2, false)])).toBe("success");
  });

  it("is error only when every provider failed", () => {
    expect(nextRefreshState([result(1, false), result(2, false)])).toBe("error");
    expect(nextRefreshState([result(1, false)])).toBe("error");
  });
});
