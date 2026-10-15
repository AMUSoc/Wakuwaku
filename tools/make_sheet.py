"""把一格一格的 PNG 排成 Wakuwaku 的圖集 spritesheet.webp（v2：1536×2288）。

用法：python tools/make_sheet.py <frames 資料夾> <輸出的 spritesheet.webp>
frames 資料夾裡每個動作一個子資料夾，檔名排序就是播放順序：
  idle/ running-right/ running-left/ waving/ jumping/ failed/
  waiting/ running/ review/ look/（look 是 16 張，00 = 正上方，順時針）
每張都要是 192×208、有透明背景（RGBA）。需要 Pillow：pip install pillow
"""
import sys
from pathlib import Path

from PIL import Image

CELL_W, CELL_H, COLS = 192, 208, 8
# 列、動作名、張數：和 src/pet/sprite.js 的 CLIPS 一致。
ROWS = [
    (0, 'idle', 6),
    (1, 'running-right', 8),
    (2, 'running-left', 8),
    (3, 'waving', 4),
    (4, 'jumping', 5),
    (5, 'failed', 8),
    (6, 'waiting', 6),
    (7, 'running', 6),
    (8, 'review', 6),
]
LOOK_ROW, LOOKS = 9, 16


def frames(folder, want):
    if not folder.is_dir():
        sys.exit(f'少了資料夾 {folder}')
    files = sorted(p for p in folder.iterdir() if p.suffix.lower() == '.png')
    if len(files) != want:
        sys.exit(f'{folder.name}：要 {want} 張，找到 {len(files)} 張')
    out = []
    for f in files:
        im = Image.open(f).convert('RGBA')
        if im.size != (CELL_W, CELL_H):
            sys.exit(f'{f}：是 {im.size[0]}×{im.size[1]}，要 {CELL_W}×{CELL_H}')
        out.append(im)
    return out


def main(src, dst):
    src = Path(src)
    sheet = Image.new('RGBA', (CELL_W * COLS, CELL_H * (LOOK_ROW + 2)), (0, 0, 0, 0))
    for row, name, want in ROWS:
        for col, im in enumerate(frames(src / name, want)):
            sheet.paste(im, (col * CELL_W, row * CELL_H))
    for i, im in enumerate(frames(src / 'look', LOOKS)):
        sheet.paste(im, ((i % COLS) * CELL_W, (LOOK_ROW + i // COLS) * CELL_H))
    sheet.save(dst, 'WEBP', lossless=True, exact=True)
    print(f'{dst}：{sheet.size[0]}×{sheet.size[1]}')


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
