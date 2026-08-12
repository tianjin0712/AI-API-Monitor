import type { Prediction } from "../types";

/** 返回无法预测耗尽时间的具体原因；数据充分时返回 null。 */
export function getPredictionUnavailableReason(
  prediction: Prediction | null,
): string | null {
  if (!prediction) return null;
  if (prediction.samples === 0) {
    return "近 7 天没有有效费用样本，暂无法预测耗尽时间。";
  }
  if (prediction.balance === null) {
    return "当前平台未提供余额，暂无法预测耗尽时间。";
  }
  if (prediction.dailyCostAvg <= 0) {
    return "近期日均消耗为 0，暂无法预测耗尽时间。";
  }
  if (prediction.daysLeft === null) {
    return "当前数据不足，暂无法预测耗尽时间。";
  }
  return null;
}
