#!/usr/bin/env bash
# PDF智转 —— GitHub 发布脚本
#
# 作用：统一版本号 → 提交 → 推送 master → 打 tag → 推送 tag 触发 GitHub Actions
#       （云端并行构建 macOS universal + Windows，完成后自动生成 Release 草稿）。
#
# 用法：
#   bash github.sh 0.2.0        # 发布 0.2.0（推荐写全 X.Y.Z）
#   bash github.sh 0.2          # 等价于 0.2.0（自动补 .0）
#   bash github.sh 0.2.0 -y     # 跳过确认，直接发布
#
# 前置：已配置名为 github 的远程（git@github.com:lifeibest/pdf-rust.git）、SSH 可连通。
# 说明：版本号会同步到 package.json / tauri.conf.json / Cargo.toml / Cargo.lock 四处，
#       其中 App 运行时显示的版本取自 Cargo.toml（后端 package.version）。

set -euo pipefail

cd "$(dirname "$0")"

die() { echo "❌ $*" >&2; exit 1; }

# ------------------------------------------------------------ 解析参数
VERSION=""
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    -y|--yes) ASSUME_YES=1 ;;
    -h|--help)
      echo "用法：bash github.sh <版本号 X.Y.Z> [-y]"; exit 0 ;;
    -*) die "未知参数：$arg" ;;
    *) VERSION="$arg" ;;
  esac
done

[[ -n "$VERSION" ]] || die "缺少版本号。用法：bash github.sh 0.2.0"

VERSION="${VERSION#v}"                                   # 容错：去掉可能的 v 前缀
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+$ ]] && VERSION="$VERSION.0"  # 0.2 → 0.2.0
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "版本号格式应为 X.Y.Z，收到：$VERSION"
TAG="v$VERSION"

# ------------------------------------------------------------ 预检
command -v git >/dev/null 2>&1 || die "未找到 git"
command -v perl >/dev/null 2>&1 || die "未找到 perl"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "当前不在 git 仓库内"

BRANCH="$(git branch --show-current)"
[[ -n "$BRANCH" ]] || die "无法确定当前分支（可能处于 detached HEAD）"

git remote get-url github >/dev/null 2>&1 \
  || die "未找到名为 github 的远程。先执行：git remote add github git@github.com:lifeibest/pdf-rust.git"

# 标签不可重复：本地或 GitHub 已存在则中止（避免覆盖已发布的版本）
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null \
  && die "本地已存在标签 $TAG，换一个版本号或先删除该标签"
if git ls-remote --exit-code --tags github "refs/tags/$TAG" >/dev/null 2>&1; then
  die "GitHub 已存在标签 $TAG，换一个版本号"
fi

echo "=============================================="
echo "  发布版本：$TAG"
echo "  分支：$BRANCH  →  远程：github"
echo "  将同步版本到：package.json / tauri.conf.json / Cargo.toml / Cargo.lock"
echo "  随后：commit → push $BRANCH → tag $TAG → push tag（触发 CI 构建）"
echo "=============================================="
if [[ "$ASSUME_YES" -ne 1 ]]; then
  read -r -p "确认发布？[y/N] " answer
  [[ "$answer" =~ ^[Yy]$ ]] || die "已取消"
fi

# ------------------------------------------------------------ 同步版本号（perl 精确替换，不重排文件）
echo "==> 写入版本号 $VERSION"
perl -0777 -pi -e 's/("version"\s*:\s*")[^"]*(")/${1}'"$VERSION"'${2}/' package.json
perl -0777 -pi -e 's/("version"\s*:\s*")[^"]*(")/${1}'"$VERSION"'${2}/' src-tauri/tauri.conf.json
perl -0777 -pi -e 's/^version = "[^"]*"/version = "'"$VERSION"'"/m' src-tauri/Cargo.toml
perl -0777 -pi -e 's/(name = "pdfai-cn"\nversion = ")[^"]*(")/${1}'"$VERSION"'${2}/' src-tauri/Cargo.lock

# 校验四处版本一致
echo "==> 校验版本一致性"
grep -q "\"version\": \"$VERSION\"" package.json           || die "package.json 版本未更新"
grep -q "\"version\": \"$VERSION\"" src-tauri/tauri.conf.json || die "tauri.conf.json 版本未更新"
grep -q "^version = \"$VERSION\"" src-tauri/Cargo.toml       || die "Cargo.toml 版本未更新"
grep -A1 'name = "pdfai-cn"' src-tauri/Cargo.lock | grep -q "version = \"$VERSION\"" \
  || die "Cargo.lock 版本未更新"
echo "    ✓ 四处版本均为 $VERSION"

# ------------------------------------------------------------ 提交
echo "==> 提交改动"
git add -A
if git diff --cached --quiet; then
  echo "    无改动需要提交（版本号此前可能已是 $VERSION）"
else
  git commit -m "chore(release): $TAG"
fi

# ------------------------------------------------------------ 推送分支
echo "==> 推送分支 $BRANCH 到 github"
git push github "$BRANCH"

# ------------------------------------------------------------ 打标签并推送（触发 CI）
echo "==> 创建并推送标签 $TAG"
git tag -a "$TAG" -m "$TAG 发布"
git push github "$TAG"

echo ""
echo "🎉 已发布 ${TAG}，GitHub Actions 构建已触发（约 13 分钟）："
echo "   构建进度：https://github.com/lifeibest/pdf-rust/actions"
echo "   Release 草稿：https://github.com/lifeibest/pdf-rust/releases"
echo "   构建完成后到 Releases 页面检查并发布草稿。"
