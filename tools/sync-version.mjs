#!/usr/bin/env node
// 将项目版本号同步为最新 git tag（去掉 v 前缀）。
// 用法：
//   node tools/sync-version.mjs          # 自动读取 `git describe --tags --abbrev=0`
//   node tools/sync-version.mjs 1.0.5    # 显式指定版本（先 bump 再打标签时使用）
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// 需要保持版本一致的文件（各自匹配 version 字段）。
const FILES = [
  { path: "package.json", pattern: /("version"\s*:\s*)"[^"]*"/ },
  { path: "src-tauri/tauri.conf.json", pattern: /("version"\s*:\s*)"[^"]*"/ },
  { path: "src-tauri/Cargo.toml", pattern: /(^version\s*=\s*)"[^"]*"/m },
  // Cargo.lock 只同步根包 ai-api-monitor 的 version，避免误改依赖版本。
  { path: "src-tauri/Cargo.lock", pattern: /(name = "ai-api-monitor"\s*\n\s*version\s*=\s*)"[^"]*"/ },
];

function normalize(value, source) {
  const version = value.replace(/^[vV]/, "").trim();
  if (!/^\d+\.\d+\.\d+(-[\w.+-]+)?$/.test(version)) {
    console.error(`✗ ${source} 不是有效的 semver 版本号：${value}`);
    process.exit(1);
  }
  return version;
}

function resolveVersion() {
  const explicit = process.argv[2];
  if (explicit) {
    return normalize(explicit, "命令行参数");
  }
  let tag;
  try {
    tag = execFileSync("git", ["describe", "--tags", "--abbrev=0"], {
      encoding: "utf8",
    }).trim();
  } catch {
    console.error(
      "✗ 未找到任何 git tag。请先打标签（如 `git tag v1.0.4`），或显式传入版本号："
    );
    console.error("  node tools/sync-version.mjs 1.0.5");
    process.exit(1);
  }
  if (!tag) {
    console.error("✗ git describe 返回空标签。");
    process.exit(1);
  }
  return normalize(tag, `标签 ${tag}`);
}

const version = resolveVersion();
let changed = false;

for (const { path, pattern } of FILES) {
  const full = join(root, path);
  const text = readFileSync(full, "utf8");
  if (!pattern.test(text)) {
    console.error(`✗ 未能在 ${path} 中找到 version 字段`);
    process.exit(1);
  }
  const next = text.replace(pattern, `$1"${version}"`);
  if (next !== text) {
    writeFileSync(full, next);
    changed = true;
    console.log(`✓ ${path} -> ${version}`);
  } else {
    console.log(`· ${path} 已是 ${version}（无变化）`);
  }
}

console.log(
  changed
    ? `\n版本号已同步为 ${version}。`
    : `\n所有文件已是最新版本 ${version}，无需修改。`
);
