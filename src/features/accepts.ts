/**
 * 可接受的文件扩展名。
 *
 * 必须与后端 `pdf/imaging.rs` 的 `IMAGE_EXTENSIONS` 保持一致，否则会出现
 * 「前端允许拖入、后端却报 unsupported」的割裂体验。
 */

export const PDF_EXTENSIONS = ["pdf"] as const;

export const IMAGE_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "webp",
  "bmp",
  "gif",
  "tif",
  "tiff",
] as const;

export const PDF_AND_IMAGE_EXTENSIONS = [
  ...PDF_EXTENSIONS,
  ...IMAGE_EXTENSIONS,
] as string[];

export function isPdfName(name: string): boolean {
  return extOf(name) === "pdf";
}

export function isImageName(name: string): boolean {
  return (IMAGE_EXTENSIONS as readonly string[]).includes(extOf(name));
}

function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
}
