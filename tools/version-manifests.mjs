import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

export const manifests = [
  { path: "package.json", pattern: /("version"\s*:\s*)"[^"]*"/ },
  { path: "src-tauri/tauri.conf.json", pattern: /("version"\s*:\s*)"[^"]*"/ },
  { path: "src-tauri/Cargo.toml", pattern: /(^version\s*=\s*)"[^"]*"/m },
  {
    path: "src-tauri/Cargo.lock",
    pattern: /(name = "ai-api-monitor"\s*\n\s*version\s*=\s*)"[^"]*"/,
  },
];

export function normalizeVersion(value, source = "版本号") {
  const version = value.replace(/^[vV]/, "").trim();
  if (!SEMVER.test(version)) {
    throw new Error(`${source} 不是有效的 SemVer：${value}`);
  }
  return version;
}

function versionFromText(text, { path, pattern }) {
  const match = text.match(pattern);
  if (!match) {
    throw new Error(`未能在 ${path} 中找到应用 version 字段`);
  }
  const value = match[0].match(/"([^"]+)"\s*$/)?.[1];
  if (!value) {
    throw new Error(`未能解析 ${path} 的应用 version 字段`);
  }
  return value;
}

export function readManifestVersions() {
  return manifests.map((manifest) => {
    const text = readFileSync(join(root, manifest.path), "utf8");
    return { path: manifest.path, version: versionFromText(text, manifest) };
  });
}

export function assertManifestVersions(expectedVersion) {
  const expected = normalizeVersion(expectedVersion);
  const versions = readManifestVersions();
  const mismatches = versions.filter(({ version }) => version !== expected);
  if (mismatches.length > 0) {
    const actual = versions.map(({ path, version }) => `${path}=${version}`).join(", ");
    throw new Error(`manifest 版本不一致；期望 ${expected}，实际 ${actual}`);
  }
  return { expected, versions };
}

export function syncManifestVersions(targetVersion) {
  const version = normalizeVersion(targetVersion);
  const changes = [];

  for (const manifest of manifests) {
    const fullPath = join(root, manifest.path);
    const text = readFileSync(fullPath, "utf8");
    if (!manifest.pattern.test(text)) {
      throw new Error(`未能在 ${manifest.path} 中找到应用 version 字段`);
    }
    const next = text.replace(manifest.pattern, `$1"${version}"`);
    if (next !== text) {
      writeFileSync(fullPath, next);
      changes.push(manifest.path);
    }
  }

  return { version, changes };
}
