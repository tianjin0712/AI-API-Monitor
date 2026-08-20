#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { assertManifestVersions, normalizeVersion } from "./version-manifests.mjs";

const args = process.argv.slice(2);
const requireTag = args.includes("--require-tag");
const values = args.filter((arg) => arg !== "--require-tag");

function git(args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

function fail(message) {
  console.error(`✗ ${message}`);
  process.exit(1);
}

if (values.length !== 1 || !values[0].startsWith("v")) {
  fail("用法：pnpm release:verify vX.Y.Z [--require-tag]");
}

const tag = values[0];
let version;
try {
  version = normalizeVersion(tag, "Tag 名");
} catch (error) {
  fail(error.message);
}

try {
  assertManifestVersions(version);
} catch (error) {
  fail(error.message);
}

if (git(["status", "--porcelain"]) !== "") {
  fail("工作区不干净；请先提交或暂存无关改动后再验证发布。");
}

const subject = git(["log", "-1", "--format=%s"]);
const expectedSubject = `chore: release ${tag}`;
if (subject !== expectedSubject) {
  fail(`HEAD 不是 release commit；期望 \"${expectedSubject}\"，实际 \"${subject}\"。`);
}

const head = git(["rev-parse", "HEAD"]);
let tagObject;
try {
  tagObject = git(["cat-file", "-t", `refs/tags/${tag}`]);
} catch {
  tagObject = "";
}

if (requireTag) {
  if (tagObject !== "tag") {
    fail(`缺少 annotated tag ${tag}。`);
  }
  if (git(["rev-list", "-n", "1", tag]) !== head) {
    fail(`${tag} 没有指向当前 release commit。`);
  }
  console.log(`✓ annotated tag ${tag} 指向 HEAD ${head}`);
} else if (tagObject) {
  fail(`Tag ${tag} 已存在；请使用 --require-tag 验证已有 Tag。`);
} else {
  console.log(`✓ ${tag} 尚不存在，可在当前 HEAD ${head} 上创建 annotated tag。`);
}

console.log(`✓ release ${tag} 的 manifest、工作区与 release commit 均符合要求。`);
