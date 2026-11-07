#!/usr/bin/env node
/**
 * 下载并安装 PDFium 动态库到 src-tauri/binaries/
 *
 * 版本必须与 src-tauri/Cargo.toml 中 pdfium-render 的 `pdfium_latest` 特性所绑定的
 * PDFium API 版本保持一致，否则运行时可能出现符号缺失。
 * 当前 pdfium-render 0.9.5 -> pdfium_latest = pdfium_8125 -> chromium/8125
 *
 * 用法:
 *   node scripts/download-pdfium.mjs            # 已存在且版本一致则跳过
 *   node scripts/download-pdfium.mjs --force    # 强制重新下载
 *   node scripts/download-pdfium.mjs --platform win32 --arch x64   # 交叉准备其他平台
 */

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** PDFium release tag，必须与 pdfium-render 的 pdfium_latest 保持一致 */
const PDFIUM_TAG = 'chromium/8125';

const REPO = 'bblanchon/pdfium-binaries';

/** 下载源前缀，依次尝试：直连 GitHub -> 加速镜像；可用环境变量 PDFIUM_MIRROR 追加自定义镜像 */
const MIRROR_PREFIXES = [
  ...(process.env.PDFIUM_MIRROR ? [process.env.PDFIUM_MIRROR] : []),
  '',
  'https://ghfast.top/',
];

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const BIN_DIR = path.join(ROOT, 'src-tauri', 'binaries');
const VERSION_FILE = path.join(BIN_DIR, 'PDFIUM_VERSION.json');

/** 每个平台对应的资产名、压缩包内的库路径、最终落盘文件名 */
const PLATFORMS = {
  darwin: {
    // mac-univ 为通用二进制，一份同时覆盖 arm64 与 x86_64
    asset: 'pdfium-mac-univ.tgz',
    entry: 'lib/libpdfium.dylib',
    output: 'libpdfium.dylib',
  },
  win32: {
    asset: 'pdfium-win-x64.tgz',
    entry: 'bin/pdfium.dll',
    output: 'pdfium.dll',
  },
  linux: {
    asset: 'pdfium-linux-x64.tgz',
    entry: 'lib/libpdfium.so',
    output: 'libpdfium.so',
  },
};

function parseArgs(argv) {
  const args = { force: false, platform: process.platform, arch: process.arch };
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--force' || token === '-f') args.force = true;
    else if (token === '--platform') args.platform = argv[(i += 1)];
    else if (token === '--arch') args.arch = argv[(i += 1)];
    else if (token === '--help' || token === '-h') {
      console.log('用法: node scripts/download-pdfium.mjs [--force] [--platform darwin|win32|linux] [--arch x64|arm64]');
      process.exit(0);
    }
  }
  return args;
}

function log(...msg) {
  console.log('[pdfium]', ...msg);
}

function warn(...msg) {
  console.warn('[pdfium]', ...msg);
}

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

function readVersionFile() {
  try {
    return JSON.parse(fs.readFileSync(VERSION_FILE, 'utf8'));
  } catch {
    return null;
  }
}

async function download(url) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);
  return Buffer.from(await res.arrayBuffer());
}

async function downloadWithFallback(assetName) {
  const encodedTag = encodeURIComponent(PDFIUM_TAG);
  let lastError = null;
  for (const prefix of MIRROR_PREFIXES) {
    const url = `${prefix}https://github.com/${REPO}/releases/download/${encodedTag}/${assetName}`;
    try {
      log(`下载 ${url}`);
      const buf = await download(url);
      if (buf.length < 1024) throw new Error('下载内容过小，疑似失败');
      return { buf, url };
    } catch (err) {
      lastError = err;
      warn(`失败: ${err.message}，尝试下一个源...`);
    }
  }
  throw lastError ?? new Error('所有下载源均失败');
}

/** 使用系统 tar 解出压缩包内的单个文件 */
function extractEntry(tgzPath, entry, destPath, workDir) {
  const extractDir = path.join(workDir, 'extract');
  fs.mkdirSync(extractDir, { recursive: true });
  const result = spawnSync('tar', ['-xzf', tgzPath, '-C', extractDir, entry], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.status !== 0) {
    throw new Error(`tar 解压失败: ${(result.stderr || result.stdout || '').toString().trim()}`);
  }
  const extracted = path.join(extractDir, entry);
  if (!fs.existsSync(extracted)) {
    throw new Error(`压缩包内未找到 ${entry}`);
  }
  fs.mkdirSync(path.dirname(destPath), { recursive: true });
  fs.copyFileSync(extracted, destPath);
  fs.rmSync(extractDir, { recursive: true, force: true });
}

function printManualGuide(spec) {
  const url = `https://github.com/${REPO}/releases/download/${encodeURIComponent(PDFIUM_TAG)}/${spec.asset}`;
  console.error(`
[pdfium] 自动下载失败，请手动完成:
  1. 打开 ${url}
  2. 解压取出 ${spec.entry}
  3. 重命名为 ${spec.output} 放到 ${path.relative(ROOT, BIN_DIR)}/
  4. 重新执行 pnpm tauri dev / pnpm tauri build

如网络受限，可设置镜像前缀后重试:
  PDFIUM_MIRROR="https://your-mirror/" node scripts/download-pdfium.mjs --force
`);
}

async function main() {
  const args = parseArgs(process.argv);
  const spec = PLATFORMS[args.platform];
  if (!spec) {
    warn(`不支持的平台 ${args.platform}，跳过 PDFium 下载。`);
    return;
  }

  const destPath = path.join(BIN_DIR, spec.output);
  const existing = readVersionFile();

  if (
    !args.force &&
    fs.existsSync(destPath) &&
    existing &&
    existing.tag === PDFIUM_TAG &&
    existing.platform === args.platform &&
    existing.arch === args.arch &&
    existing.output === spec.output
  ) {
    log(`${spec.output} 已就位 (${PDFIUM_TAG})，跳过下载。使用 --force 可强制重新下载。`);
    return;
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });
  const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'pdfium-'));
  const tgzPath = path.join(workDir, spec.asset);

  try {
    const { buf, url } = await downloadWithFallback(spec.asset);
    fs.writeFileSync(tgzPath, buf);
    log(`下载完成 ${(buf.length / 1024 / 1024).toFixed(2)} MB，sha256=${sha256(buf).slice(0, 16)}...`);

    extractEntry(tgzPath, spec.entry, destPath, workDir);
    fs.chmodSync(destPath, 0o755);

    const libBuf = fs.readFileSync(destPath);
    const record = {
      tag: PDFIUM_TAG,
      asset: spec.asset,
      entry: spec.entry,
      output: spec.output,
      platform: args.platform,
      arch: args.arch,
      sha256: sha256(libBuf),
      bytes: libBuf.length,
      source: url,
      downloadedAt: new Date().toISOString(),
    };
    fs.writeFileSync(VERSION_FILE, `${JSON.stringify(record, null, 2)}\n`);

    log(`已安装 ${path.relative(ROOT, destPath)} (${(libBuf.length / 1024 / 1024).toFixed(2)} MB)`);
  } catch (err) {
    warn(err.message);
    printManualGuide(spec);
    // 已有可用库时仅告警，避免因网络抖动阻断开发；完全缺失时以非零码退出便于感知
    if (!fs.existsSync(destPath)) process.exitCode = 1;
  } finally {
    fs.rmSync(workDir, { recursive: true, force: true });
  }
}

main();
