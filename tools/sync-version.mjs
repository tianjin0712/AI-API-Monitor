#!/usr/bin/env node
// 版本同步必须发生在 release commit 之前，绝不根据既有 Tag 回写 manifest。
import { syncManifestVersions } from "./version-manifests.mjs";

const [targetVersion] = process.argv.slice(2);

if (!targetVersion || process.argv.length !== 3) {
  console.error("用法：pnpm version:sync <X.Y.Z>");
  console.error("示例：pnpm version:sync 1.0.7");
  process.exit(1);
}

try {
  const { version, changes } = syncManifestVersions(targetVersion);
  if (changes.length === 0) {
    console.log(`所有受控 manifest 已是 ${version}。`);
  } else {
    console.log(`已同步 ${version}：${changes.join(", ")}`);
  }
} catch (error) {
  console.error(`✗ ${error.message}`);
  process.exit(1);
}
