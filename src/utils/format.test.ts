import { afterEach, describe, expect, it, vi } from "vitest";
import { formatCount, formatMoney, formatRelativeTime } from "./format";

describe("formatCount", () => {
  it("formats small numbers without abbreviation", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
  });

  it("abbreviates thousands and millions", () => {
    expect(formatCount(1234)).toBe("1.2K");
    expect(formatCount(1_000_000)).toBe("1.0M");
    expect(formatCount(1_234_567)).toBe("1.2M");
  });

  it("renders non-finite input as a dash", () => {
    expect(formatCount(Number.NaN)).toBe("—");
    expect(formatCount(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("formatMoney", () => {
  it("renders null / undefined / non-finite as a dash", () => {
    expect(formatMoney(null)).toBe("—");
    expect(formatMoney(undefined)).toBe("—");
    expect(formatMoney(Number.NaN)).toBe("—");
  });

  it("formats with up to two decimals and locale grouping", () => {
    expect(formatMoney(100)).toBe("100");
    expect(formatMoney(0.5)).toBe("0.5");
    expect(formatMoney(12.345)).toBe("12.35");
    expect(formatMoney(1234.5)).toBe("1,234.5");
  });
});

describe("formatRelativeTime", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders a dash for missing or invalid input", () => {
    expect(formatRelativeTime(null)).toBe("—");
    expect(formatRelativeTime(undefined)).toBe("—");
    expect(formatRelativeTime("not-a-date")).toBe("—");
  });

  it("renders 刚刚 within a minute", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:00Z"));
    expect(formatRelativeTime("2026-08-14T11:59:40Z")).toBe("刚刚");
  });

  it("renders minutes for the last hour", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:00Z"));
    expect(formatRelativeTime("2026-08-14T11:55:00Z")).toBe("5 分钟前");
  });

  it("renders hours within a day", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:00Z"));
    expect(formatRelativeTime("2026-08-14T10:00:00Z")).toBe("2 小时前");
  });

  it("falls back to a date string beyond a day", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-14T12:00:00Z"));
    const past = "2026-08-10T00:00:00Z";
    expect(formatRelativeTime(past)).toBe(new Date(past).toLocaleDateString("zh-CN"));
  });
});
