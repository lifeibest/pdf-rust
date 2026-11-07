#!/usr/bin/env python3
"""把 AI 生成的图标原图清理成可用于 `pnpm tauri icon` 的源图。

原图有两个问题：右下角带「AI 生成」水印，且背景是不透明的浅灰而不是透明。
这里做两件事：

1. 按固定框裁掉水印所在的角落（squircle 之外的区域）；
2. 从画布边缘做连通域泛洪，找出「与背景相连」的区域，再按与背景色的距离做
   平滑 alpha 渐变 —— 既去掉柔和投影，又不会误伤 squircle 内部的白色图形
   （白色与浅灰背景的颜色距离很近，只按颜色抠图会把图形也抠掉）。

用法：python3 scripts/make-icon-source.py <in.png> <out.png>
"""

import sys

from PIL import Image, ImageDraw

# 原图 1024x1024，squircle 大致落在 [110, 914]，水印在右下角之外
CROP_BOX = (96, 96, 928, 928)
BACKGROUND = (242, 245, 247)
# 与背景色的距离小于 FLOOR 视为纯背景，大于 CEIL 视为完全不透明，中间线性过渡
ALPHA_FLOOR = 14.0
ALPHA_CEIL = 70.0
TARGET = 1024


def distance(pixel: tuple[int, int, int]) -> float:
    return sum((a - b) ** 2 for a, b in zip(pixel, BACKGROUND)) ** 0.5


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)

    source, destination = sys.argv[1], sys.argv[2]
    image = Image.open(source).convert("RGB").crop(CROP_BOX)
    width, height = image.size
    pixels = image.load()

    # 二值图：与背景足够接近的像素标为白，其余标为黑
    bglike = Image.new("L", (width, height), 0)
    bg_pixels = bglike.load()
    for y in range(height):
        for x in range(width):
            if distance(pixels[x, y]) <= ALPHA_CEIL:
                bg_pixels[x, y] = 255

    # 从左上角泛洪：只有与画布边缘连通的浅色区域才算背景，
    # squircle 内部的白色图形因此被完整保留
    ImageDraw.floodfill(bglike, (0, 0), 128, thresh=1)

    alpha = Image.new("L", (width, height), 255)
    alpha_pixels = alpha.load()
    for y in range(height):
        for x in range(width):
            if bg_pixels[x, y] != 128:
                continue
            dist = distance(pixels[x, y])
            ratio = (dist - ALPHA_FLOOR) / (ALPHA_CEIL - ALPHA_FLOOR)
            alpha_pixels[x, y] = int(max(0.0, min(1.0, ratio)) * 255)

    image.putalpha(alpha)

    # 居中放回 1024 画布，四周留出与 macOS 图标规范相符的空白
    canvas = Image.new("RGBA", (TARGET, TARGET), (0, 0, 0, 0))
    offset = (TARGET - width) // 2
    canvas.paste(image, (offset, offset), image)
    canvas.save(destination, "PNG")
    print(f"wrote {destination} ({TARGET}x{TARGET})")


if __name__ == "__main__":
    main()
