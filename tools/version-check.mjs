#!/usr/bin/env node
import { assertManifestVersions, normalizeVersion, readManifestVersions } from "./version-manifests.mjs";

const [requestedVersion] = process.argv.slice(2);

try {
  const expected = requestedVersion
    ? normalizeVersion(requestedVersion, "命令行版本")
    : readManifestVersions()[0].version;
  const { versions } = assertManifestVersions(expected);
  for (const { path, version } of versions) {
    console.log(`✓ ${path} = ${version}`);
  }
  console.log(`manifest 版本一致：${expected}`);
} catch (error) {
  console.error(`✗ ${error.message}`);
  process.exit(1);
}
