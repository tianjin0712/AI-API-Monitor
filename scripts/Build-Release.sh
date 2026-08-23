#!/usr/bin/env bash
# macOS 打包脚本（scripts/Build-Release.ps1 的 macOS 端等价物）。
#
# 效果与 Windows 脚本一致：
#   1. 以 Git Tag 作为唯一版本来源：精确 tag > 最近可达 tag > package.json 回退；
#   2. 复用 pnpm version:sync 把版本注入 manifest（应用内部版本与文件名同源）；
#   3. 产物文件名末尾追加版本号；
#   4. 打包到输出目录（默认 ~/release）。
#
# 仅处理 macOS 产物（.app + .dmg）；macOS 无法交叉编译 Windows .exe。
set -euo pipefail

PRODUCT_NAME="AI API Monitor"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SKIP_CHECKS=0
OUTPUT_DIR="$HOME/release"

usage() {
    echo "用法: $0 [--skip-checks] [--output-dir <目录>]"
    echo "  默认输出目录: ~/release"
    echo "  版本来源: Git Tag（精确 tag > 最近 tag）> package.json 回退"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-checks) SKIP_CHECKS=1; shift ;;
        --output-dir)
            OUTPUT_DIR="${2:-}"
            [[ -z "$OUTPUT_DIR" ]] && { echo "缺少 --output-dir 参数" >&2; usage; exit 1; }
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) echo "未知参数: $1" >&2; usage; exit 1 ;;
    esac
done

cd "$PROJECT_ROOT"

# ---- 版本解析（与 tools/package-windows.mjs 的 resolveVersionFromGit 一致）----
resolve_version_from_git() {
    local exact nearest
    if exact="$(git describe --tags --exact-match HEAD 2>/dev/null)"; then
        printf '%s\n' "$exact"
        return 0
    fi
    if nearest="$(git describe --tags --abbrev=0 HEAD 2>/dev/null)"; then
        printf '%s\n' "$nearest"
        return 0
    fi
    return 1
}

# 去 v 前缀，得到与 manifest 内部一致的 SemVer（v1.2.3 -> 1.2.3）。
normalize_version() {
    node -e 'process.stdout.write(process.argv[1].replace(/^[vV]/, "").trim())' "$1"
}

# 仅用于最终文件名的安全处理，不改变版本语义。
sanitize_filename() {
    node -e 'process.stdout.write(process.argv[1].replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-"))' "$1"
}

# 回退版本：沿用现有 manifest 机制（package.json），与 package-windows.mjs 的 fallback 一致。
manifest_fallback_version() {
    node -p "require('./package.json').version"
}

# 架构标记（Tauri 命名惯例：arm64 -> aarch64，x86_64 -> x64）。
arch_name() {
    case "$(uname -m)" in
        arm64|aarch64) printf 'aarch64' ;;
        x86_64|amd64)  printf 'x64' ;;
        *)             uname -m ;;
    esac
}

# ---- 1. 解析版本：优先 Git Tag，获取失败回退 package.json ----
if git_tag="$(resolve_version_from_git)"; then
    VERSION="$(normalize_version "$git_tag")"
    VERSION_SOURCE="Git Tag（$git_tag）"
else
    VERSION="$(normalize_version "$(manifest_fallback_version)")"
    VERSION_SOURCE="fallback（package.json，未检测到 Git Tag）"
fi
SAFE_VERSION="$(sanitize_filename "$VERSION")"
ARCH="$(arch_name)"
echo "版本来源：$VERSION_SOURCE -> $VERSION"

# ---- 2. 注入 manifest，保证「应用内部版本」与「产物文件名」同源一致 ----
pnpm version:sync "$VERSION"

# ---- 3. 质量检查 ----
if [[ "$SKIP_CHECKS" -ne 1 ]]; then
    pnpm check
fi

# ---- 4. 构建 macOS bundle（--bundles 覆盖配置里的 nsis，产出 app + dmg）----
pnpm tauri build --bundles app,dmg

# ---- 5. 收集产物并重命名（版本在末尾），复制到输出目录 ----
DMG_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle/dmg"
DMG_FILE="$(find "$DMG_DIR" -maxdepth 1 -name '*.dmg' -print -quit 2>/dev/null)"
if [[ -z "$DMG_FILE" ]]; then
    echo "✗ 未找到 DMG 产物：$DMG_DIR" >&2
    exit 1
fi

mkdir -p "$OUTPUT_DIR"
FINAL_DMG="$OUTPUT_DIR/${PRODUCT_NAME}_${ARCH}_${SAFE_VERSION}.dmg"
cp -f "$DMG_FILE" "$FINAL_DMG"

# 同时复制 .app（可直接运行），保持 bundle 名不变（版本已注入内部）。
APP_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle/macos"
APP_FILE="$(find "$APP_DIR" -maxdepth 1 -name '*.app' -print -quit 2>/dev/null)"
if [[ -n "$APP_FILE" ]]; then
    rm -rf "$OUTPUT_DIR/${PRODUCT_NAME}.app"
    cp -R "$APP_FILE" "$OUTPUT_DIR/${PRODUCT_NAME}.app"
fi

echo "Release artifacts created: $OUTPUT_DIR"
echo "  - $FINAL_DMG"
