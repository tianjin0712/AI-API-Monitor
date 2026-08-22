#!/usr/bin/env node
// Windows 打包脚本（仅 Windows / NSIS）。
//
// 目标：
// 1. 打包时自动根据当前 Git Tag 派生版本（不硬编码）：
//    - HEAD 恰好位于某个 tag（正式发布）→ 用该 tag；
//    - 非 tag 提交 → 用 HEAD 可达的最近 tag；
//    - 无 tag / Git 不可用 / tag 获取失败 → 回退到 package.json 的 version（现有 manifest 机制）。
// 2. 把该版本注入 manifest（复用 version-manifests.mjs 的 syncManifestVersions），
//    保证「产物文件名」与「应用内部版本」同源一致：
//    - Cargo.toml 的 version → 编译期 `env!("CARGO_PKG_VERSION")`（Rust 后端）；
//    - tauri.conf.json 的 version → `app.version()` 与 bundle 产物命名。
// 3. 执行 `tauri build`，随后把 NSIS 安装包重命名为「版本名在末尾」的最终文件名。
//
// 本脚本只处理 Windows 打包，不涉及 macOS。

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, renameSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { root, normalizeVersion, syncManifestVersions } from "./version-manifests.mjs";

// 与 docs/RELEASE.md 保持一致的产品名与架构标记。
const PRODUCT_NAME = "AI API Monitor";
const ARCH = "x64";
// tauri build（NSIS target）的固定输出目录。
const NSIS_DIR = join(root, "src-tauri", "target", "release", "bundle", "nsis");

function tryGit(args) {
  try {
    return execFileSync("git", args, {
      encoding: "utf8",
      cwd: root,
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return "";
  }
}

/** 回退：读取 package.json 的 version（现有 manifest 单一来源）。 */
export function manifestFallbackVersion() {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  return normalizeVersion(pkg.version, "package.json 版本");
}

/**
 * 从 Git Tag 派生版本。返回 { version, source }；解析不到（无 tag / git 失败）返回 null。
 * version 为去 v 前缀的 SemVer（如 v1.2.3 -> 1.2.3），与 manifest 内部版本一致。
 */
export function resolveVersionFromGit() {
  const exact = tryGit(["describe", "--tags", "--exact-match", "HEAD"]);
  if (exact) {
    return { version: normalizeVersion(exact, "Git Tag"), source: exact };
  }
  const nearest = tryGit(["describe", "--tags", "--abbrev=0", "HEAD"]);
  if (nearest) {
    return { version: normalizeVersion(nearest, "Git Tag"), source: `最近的 tag ${nearest}` };
  }
  return null;
}

/** Windows 文件名非法字符与控制字符的最小必要清洗（SemVer 通常不含这些，双保险）。 */
export function sanitizeFileName(value) {
  // eslint-disable-next-line no-control-regex
  return value.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-");
}

/** 最终产物文件名：保留产品名、架构与安装器类型，版本名置于末尾。 */
export function finalArtifactName(version) {
  return `${PRODUCT_NAME}_${ARCH}-setup_${sanitizeFileName(version)}.exe`;
}

function fail(message) {
  console.error(`✗ ${message}`);
  process.exit(1);
}

function runPackage() {
  const resolved = resolveVersionFromGit();
  const version = resolved ? resolved.version : manifestFallbackVersion();
  const source = resolved ? `Git Tag（${resolved.source}）` : "fallback（package.json，未检测到 Git Tag）";
  console.log(`[package-windows] 版本来源：${source} → ${version}`);

  // 注入 manifest（幂等）：确保 Cargo.toml / tauri.conf.json 的 version 与产物名同源一致。
  const { changes } = syncManifestVersions(version);
  if (changes.length > 0) {
    console.log(`[package-windows] 已同步 manifest 版本：${changes.join(", ")}`);
  }

  console.log("[package-windows] 开始 tauri build（Windows / NSIS）…");
  execFileSync("pnpm", ["tauri", "build"], {
    cwd: root,
    stdio: "inherit",
    // Windows 上 pnpm 是 .cmd/.ps1，需要 shell 解析；其他平台直接执行。
    shell: process.platform === "win32",
  });

  if (!existsSync(NSIS_DIR)) {
    fail(`未找到 NSIS 产物目录：${NSIS_DIR}`);
  }
  const setupFile = readdirSync(NSIS_DIR).find((name) => name.endsWith("-setup.exe"));
  if (!setupFile) {
    fail(`未在 ${NSIS_DIR} 找到 NSIS 安装包（*-setup.exe）。`);
  }
  const target = finalArtifactName(version);
  const from = join(NSIS_DIR, setupFile);
  const to = join(NSIS_DIR, target);
  renameSync(from, to);
  console.log(`[package-windows] 最终产物：${to}`);
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  runPackage();
}
