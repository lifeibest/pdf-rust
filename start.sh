#!/usr/bin/env bash
# PDF智转 —— 本地测试运行（开发模式）一键脚本
#
# 作用：准备依赖 → 下载 PDFium 引擎 → 启动 Tauri 开发模式（Vite 热重载 + Rust 增量编译）。
# 用法：
#   bash start.sh          # 常规启动（依赖已就绪时会自动跳过重复步骤）
#   bash start.sh --force  # 强制重新下载 PDFium 引擎后再启动
#
# 说明：所有 PDF 计算在 Rust 侧完成，运行前必须有对应平台的 PDFium 动态库，
#       因此 pnpm tauri dev 会经 pretauri 钩子自动下载；此处再显式跑一次以 fail-fast。

set -euo pipefail

# 切到脚本所在目录（项目根），保证从任意位置调用都正确
cd "$(dirname "$0")"

FORCE=""
if [[ "${1:-}" == "--force" ]]; then
  FORCE="--force"
fi

echo "==> 检查 pnpm"
if ! command -v pnpm >/dev/null 2>&1; then
  echo "未检测到 pnpm，请先安装：npm install -g pnpm@9" >&2
  exit 1
fi

echo "==> 安装前端依赖（node_modules 缺失时）"
if [[ ! -d node_modules ]]; then
  pnpm install
else
  echo "    node_modules 已存在，跳过 pnpm install"
fi

echo "==> 下载 PDFium 引擎（已存在会自动跳过${FORCE:+，本次强制重下}）"
pnpm pdfium ${FORCE}

echo "==> 启动开发模式：pnpm tauri dev"
echo "    关闭窗口或按 Ctrl+C 结束。"
pnpm tauri dev
