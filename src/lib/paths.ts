/**
 * 纯字符串的路径工具。
 *
 * 本期只跑 macOS，分隔符固定为 `/`；Windows 分支（`\`）先保留判断，等后续接入时
 * 不至于全线出错。所有函数都不触碰文件系统。
 */

const SEPARATORS = ["/", "\\"];

function lastSeparator(path: string): number {
  let index = -1;
  for (const sep of SEPARATORS) {
    const found = path.lastIndexOf(sep);
    if (found > index) index = found;
  }
  return index;
}

/** 文件名部分：`/a/b/c.pdf` → `c.pdf` */
export function basename(path: string): string {
  const index = lastSeparator(path);
  return index < 0 ? path : path.slice(index + 1);
}

/** 所在目录：`/a/b/c.pdf` → `/a/b` */
export function dirname(path: string): string {
  const index = lastSeparator(path);
  if (index < 0) return ".";
  if (index === 0) return "/";
  return path.slice(0, index);
}

/** 小写扩展名（不含点）：`a.PDF` → `pdf` */
export function extname(path: string): string {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
}

/** 去掉扩展名的文件名：`报告 v1.pdf` → `报告 v1` */
export function stem(path: string): string {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? name : name.slice(0, dot);
}

/** 拼接目录与文件名，自动处理多余/缺失的分隔符。 */
export function joinPath(dir: string, name: string): string {
  if (!dir) return name;
  if (!name) return dir;
  const tail = dir.slice(-1);
  return SEPARATORS.includes(tail) ? `${dir}${name}` : `${dir}/${name}`;
}

/** 把绝对路径截断为「父目录 / …  / 文件名」形式，用于列表里显示来源。 */
export function shortenPath(path: string, maxDir = 42): string {
  const dir = dirname(path);
  if (dir.length <= maxDir) return path;
  return `…${dir.slice(-maxDir)}/${basename(path)}`;
}
