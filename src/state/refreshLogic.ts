import type { ProviderUsage, RefreshResult, RefreshSettings } from "../types";

/**
 * 刷新调度与结果合并的纯函数（从 MonitorStore 提取，便于单元测试）。
 * 与原 store 逻辑保持语义完全一致：合并仍以「函数式更新」方式作用于最新状态。
 */

/** 计算下次刷新的间隔秒数：前台最小 10s、后台最小 60s（与 store 原逻辑一致）。 */
export function computeRefreshIntervalSecs(
  settings: RefreshSettings,
  visible: boolean,
): number {
  return visible
    ? Math.max(settings.foregroundSecs, 10)
    : Math.max(settings.backgroundSecs, 60);
}

/** 把成功的用量结果合并进 usages；providerId 缺失的结果不写入。 */
export function mergeUsageResults(
  current: Record<number, ProviderUsage>,
  results: RefreshResult[],
): Record<number, ProviderUsage> {
  const next = { ...current };
  for (const result of results) {
    if (result.success && result.usage?.providerId != null) {
      next[result.usage.providerId] = result.usage;
    }
  }
  return next;
}

/** 把刷新结果合并进 errors：成功清除该账户错误，失败写入错误信息。 */
export function mergeErrorResults(
  current: Record<number, string>,
  results: RefreshResult[],
): Record<number, string> {
  const next = { ...current };
  for (const result of results) {
    if (result.success) {
      delete next[result.providerId];
    } else {
      next[result.providerId] = result.error ?? "刷新失败";
    }
  }
  return next;
}

/** 是否存在至少一个「成功且带用量数据」的账户。 */
export function refreshSucceeded(results: RefreshResult[]): boolean {
  return results.some(
    (result) => result.success && result.usage?.providerId != null,
  );
}

/** 是否存在至少一个失败账户。 */
export function anyRefreshFailed(results: RefreshResult[]): boolean {
  return results.some((result) => !result.success);
}

/** 刷新状态判定：有失败且无任何成功数据 → error，否则 success。 */
export function nextRefreshState(results: RefreshResult[]): "success" | "error" {
  return anyRefreshFailed(results) && !refreshSucceeded(results)
    ? "error"
    : "success";
}
