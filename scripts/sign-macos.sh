#!/usr/bin/env bash
#
# 打包后对 macOS 产物做 ad-hoc 签名。
#
# Tauri 在 `signingIdentity: null` 时不会对 bundle 签名，而内嵌的
# `libpdfium.dylib` 是运行时 `dlopen` 加载的：没有有效签名的动态库会被
# Gatekeeper / AMFI 直接拒绝加载，表现为「应用能启动，但所有 PDF 功能报
# pdfiumNotFound」。因此必须：
#
#   1. 先签 Contents/Resources 下的每个 dylib（带 hardened runtime）
#   2. 再整体签 .app（外层签名覆盖内层）
#
# 用法：
#   pnpm sign:macos                     # 自动找最新构建出的 .app
#   pnpm sign:macos path/to/PDF智转.app  # 指定 .app
#   pnpm sign:macos --dmg               # 签名后用已签名的 .app 重建 dmg
#
# `--dmg` 存在的原因：`pnpm tauri build` 产出的 dmg 里装的是**未签名**的 .app，
# 而 tauri 在只打 dmg 时会把中间产物 .app 清掉，无法「先签再装进 dmg」。
# 因此这里用 hdiutil 直接从已签名的 .app 重建镜像（覆盖 tauri 的产物）。
# 代价是失去 tauri.conf.json 里的 dmg 窗口布局（图标位置 / 背景图），
# 换取「安装出来的就是已签名的应用」。
#
# 本期不做 Apple 公证（需要付费开发者账号），产物只适合本机与团队内分发。
# 接入正式签名与公证的改法见 docs/BUILD.md。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE_DIRS=("$ROOT/src-tauri/target/release/bundle/macos" "$ROOT/src-tauri/target/debug/bundle/macos")
DMG_DIRS=("$ROOT/src-tauri/target/release/bundle/dmg" "$ROOT/src-tauri/target/debug/bundle/dmg")

WITH_DMG=0
if [[ "${1:-}" == "--dmg" ]]; then
  WITH_DMG=1
  shift
fi

log() { printf '\033[36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[31m错误：\033[0m%s\n' "$*" >&2; exit 1; }

find_app() {
  local dir candidate
  for dir in "${BUNDLE_DIRS[@]}"; do
    [[ -d "$dir" ]] || continue
    # 取修改时间最新的一个：universal 构建会同时留下多个 .app
    candidate="$(find "$dir" -maxdepth 1 -name '*.app' -print0 2>/dev/null \
      | xargs -0 ls -dt 2>/dev/null | head -n 1 || true)"
    [[ -n "$candidate" ]] && { printf '%s' "$candidate"; return 0; }
  done
  return 1
}

APP="${1:-}"
if [[ -z "$APP" ]]; then
  APP="$(find_app)" || die "没找到 .app，请先执行 pnpm tauri build"
fi
[[ -d "$APP" ]] || die "路径不存在或不是 .app：$APP"

command -v codesign >/dev/null 2>&1 || die "找不到 codesign，请确认已安装 Xcode Command Line Tools"

log "签名目标：$APP"

# 1) 内嵌动态库。Resources 下可能同时存在多个（例如未来的 pdfium.dll 不会出现在
#    macOS 产物里，但保留通配以便后续加入其它 dylib）。
shopt -s nullglob
DYLIBS=("$APP/Contents/Resources/"*.dylib "$APP/Contents/Frameworks/"*.dylib)
shopt -u nullglob

if [[ ${#DYLIBS[@]} -eq 0 ]]; then
  log "警告：没有找到任何内嵌 dylib，PDF 功能可能无法工作"
fi

for dylib in "${DYLIBS[@]}"; do
  log "签名 $(basename "$dylib")"
  codesign --force --sign - --options runtime --timestamp=none "$dylib"
done

# 2) 应用本体。--deep 会把嵌套的 framework/helper 一并签掉。
log "签名 $(basename "$APP")"
codesign --force --deep --sign - --timestamp=none "$APP"

log "校验签名"
codesign --verify --deep --strict --verbose=2 "$APP"

# 3) 可选：用已签名的 .app 重建 dmg，覆盖 tauri 的产物
if [[ "$WITH_DMG" -eq 1 ]]; then
  log "重建 dmg"
  OUT=""
  for dir in "${DMG_DIRS[@]}"; do
    [[ -d "$dir" ]] || continue
    OUT="$(find "$dir" -maxdepth 1 -name '*.dmg' -print 2>/dev/null | head -n 1 || true)"
    [[ -n "$OUT" ]] && break
  done
  if [[ -z "$OUT" ]]; then
    OUT="${BUNDLE_DIRS[0]}/$(basename "$APP").dmg"
    mkdir -p "$(dirname "$OUT")"
  fi

  STAGING="$(mktemp -d)"
  trap 'rm -rf "$STAGING"' EXIT
  cp -R "$APP" "$STAGING/"
  ln -s /Applications "$STAGING/Applications"

  hdiutil create \
    -volname "$(basename "$APP" .app)" \
    -srcfolder "$STAGING" \
    -ov -format UDZO -imagekey zlib-level=9 \
    "$OUT" >/dev/null
  codesign --force --sign - --timestamp=none "$OUT"
  log "dmg 已写入：$OUT"
fi

log "完成。首次运行如被 Gatekeeper 拦截，执行：xattr -dr com.apple.quarantine \"$APP\""
