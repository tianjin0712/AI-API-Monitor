---
name: ai-api-monitor-quality-gate
description: Validate AI API Monitor changes with repeatable environment, Git diff, TypeScript, Vitest, Rust formatting, Clippy, Rust tests, frontend build, secret-file, and optional dependency-audit checks. Use after changing the AI API Monitor React/Tauri project, before handing off work, committing, opening a PR, building a release, or when asked whether the project is healthy or ready to ship.
---

# AI API Monitor Quality Gate

Run a consistent, non-destructive validation pass and return an evidence-based handoff verdict.

## Workflow

1. Confirm the repository by locating `package.json`, `src-tauri/Cargo.toml`, and `.github/workflows/quality.yml`.
2. Inspect `git status --short` and `git diff --check`. Preserve all existing changes.
3. Run `scripts/quality-gate.ps1` from the skill directory with `-ProjectRoot <path>`.
4. Add `-SecurityAudit` when dependencies, network code, credentials, storage, permissions, updater configuration, or release artifacts changed.
5. Add `-TauriBuild` only for release readiness or when packaging, permissions, icons, capabilities, platform code, or Tauri configuration changed. This can be slow and creates build artifacts.
6. For UI work, perform actual rendered-page regression separately. Check Full mode at 460x720, light/dark themes, overflow, navigation, settings controls, and browser console errors. Do not claim UI verification from compilation alone.
7. Report PASS, FAIL, or PARTIAL. FAIL blocks handoff. PARTIAL means required platform, credentials, interactive desktop behavior, or tooling was unavailable.

## Safety Rules

- Do not install dependencies unless the user requested setup or repair.
- Do not use real API keys in tests.
- Do not print credential values, environment secrets, browser storage, or keyring contents.
- Do not stage, commit, clean, reset, publish, or release as part of validation.
- Treat dependency audit findings as requiring review; do not run automatic force-fixes.
- If `cargo-audit` is missing, report PARTIAL and provide `cargo install cargo-audit --locked`; do not install it without authorization.
- Stop and report the first failed deterministic command, while preserving its useful error output.

## Verdict Requirements

Include:

- Scope and changed-file categories.
- Exact checks run and their results.
- Checks skipped and why.
- First actionable failure, if any.
- Final verdict: `PASS`, `FAIL`, or `PARTIAL`.
- Remaining manual checks, especially tray/window behavior, real Provider access, OS credential stores, and platform packaging.

Read `references/check-matrix.md` when deciding which optional checks are required.
