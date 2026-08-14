import { describe, expect, it } from "vitest";
import type { Prediction } from "../types";
import { getPredictionUnavailableReason } from "./prediction";

const validPrediction: Prediction = {
  dailyCostAvg: 2,
  samples: 4,
  daysSpan: 7,
  balance: 20,
  daysLeft: 10,
  exhaustedDate: "2026-08-23",
};

describe("getPredictionUnavailableReason", () => {
  it("accepts a complete prediction", () => {
    expect(getPredictionUnavailableReason(validPrediction)).toBeNull();
  });

  it("explains missing samples before other missing fields", () => {
    expect(
      getPredictionUnavailableReason({
        ...validPrediction,
        samples: 0,
        balance: null,
        daysLeft: null,
      }),
    ).toContain("没有有效费用样本");
  });

  it("explains missing balance", () => {
    expect(
      getPredictionUnavailableReason({
        ...validPrediction,
        balance: null,
        daysLeft: null,
      }),
    ).toContain("未提供余额");
  });

  it("explains zero average cost", () => {
    expect(
      getPredictionUnavailableReason({
        ...validPrediction,
        dailyCostAvg: 0,
        daysLeft: null,
      }),
    ).toContain("日均消耗为 0");
  });

  it("explains missing daysLeft when everything else is sufficient", () => {
    expect(
      getPredictionUnavailableReason({
        ...validPrediction,
        daysLeft: null,
      }),
    ).toContain("数据不足");
  });

  it("returns null for a null prediction", () => {
    expect(getPredictionUnavailableReason(null)).toBeNull();
  });
});
