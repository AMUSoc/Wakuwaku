"""把 3×3 的 Q 版姿勢表切成 Wakuwaku 寵物的各列幀，排成圖集（docs/MAKING_A_PET.md）。

用法：
  python tools/slice_sheet.py <姿勢表.png> <輸出資料夾> [--run-b 檔案] [--id 寵物id] [--name 顯示名]
                              [--fit same|each] [--preview 預覽.png]

姿勢表：3×3，透明背景，格子之間是透明區域，由左到右、由上到下是
  idle-a、idle-b、wave、jump、failed、waiting、working、review、run-a
--run-b：第二張跑步幀（一張圖，透明背景）；沒有就全部用 run-a。

做的事：
  1. 用透明區域的投影切出 9 格（先切列，再在每一列裡切欄），各自裁掉多餘的透明邊。
     AI 繪圖的背景常帶 alpha 只有 1–7 的雜點，判斷透明時忽略 alpha < 8 的點（--alpha），
     並把它們清成全透明；alpha ≥ 240 的當成不透明（255）。
  2. 等比縮放到 180×200 裡（寬、高都要放得下）。--fit same（預設）：9 格用同一個
     比例（最難塞的那格決定），換姿勢時角色大小不變；--fit each：每格各自塞滿。
     貼到 192×208 的透明畫布，左右置中，腳底（不透明部分的最下緣）對齊 y=204，
     jump 對齊 y=196。
  3. 排成各列資料夾（<輸出資料夾>/idle/00.png …），running-left 是 running-right 的水平鏡像。
  4. 呼叫 make_sheet.py 產出 data/pets/<id>/spritesheet.webp，並寫 pet.json。
需要 Pillow 和 NumPy：pip install pillow numpy
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageOps

sys.path.insert(0, str(Path(__file__).parent))
import make_sheet  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
POSES = ['idle-a', 'idle-b', 'wave', 'jump', 'failed', 'waiting', 'working', 'review', 'run-a']
CELL_W, CELL_H = make_sheet.CELL_W, make_sheet.CELL_H
FIT_W, FIT_H = 180, 200
FEET_Y = {'jump': 196}
FEET_Y_DEFAULT = 204
DESCRIPTION = '角色設計參考自 B 站 UP 主「沉默寡言佰佰」的 Grok 4.7 形象，非官方 Q 版二創。'


def clean(im, floor):
    """alpha < floor 的雜點清成全透明，≥ 240 的當成不透明。"""
    a = np.array(im.getchannel('A'))
    a[a < floor] = 0
    a[a >= 240] = 255
    out = im.copy()
    out.putalpha(Image.fromarray(a))
    return out


def runs(filled):
    """一條投影上連續有東西的區段 [(起, 止)]。"""
    out, start = [], None
    for i, on in enumerate(filled):
        if on and start is None:
            start = i
        elif not on and start is not None:
            out.append((start, i))
            start = None
    if start is not None:
        out.append((start, len(filled)))
    return out


def three(filled, what):
    """切成三段：有東西的區段多於三段時（小碎點），併到最近的大段。"""
    parts = runs(filled)
    while len(parts) > 3:
        # 最小的一段併進相鄰、間隔較近的那段。
        i = min(range(len(parts)), key=lambda k: parts[k][1] - parts[k][0])
        left = parts[i][0] - parts[i - 1][1] if i > 0 else None
        right = parts[i + 1][0] - parts[i][1] if i + 1 < len(parts) else None
        j = i - 1 if right is None or (left is not None and left <= right) else i + 1
        a, b = sorted((i, j))
        parts[a:b + 1] = [(parts[a][0], parts[b][1])]
    if len(parts) != 3:
        sys.exit(f'{what}：透明區域只分出 {len(parts)} 段，要 3 段；格子之間要留透明的間隔')
    return parts


def cut(sheet):
    """姿勢表切成 9 格，各自裁到不透明部分的邊。"""
    a = np.array(sheet.getchannel('A')) > 0
    cells = []
    for y0, y1 in three(a.any(axis=1), '列'):
        band = a[y0:y1]
        for x0, x1 in three(band.any(axis=0), f'第 {len(cells) // 3 + 1} 列的欄'):
            cell = sheet.crop((x0, y0, x1, y1))
            cells.append(cell.crop(cell.getbbox()))
    return cells


def scale_of(im):
    return min(FIT_W / im.width, FIT_H / im.height)


def place(im, scale, feet_y):
    """等比縮放後貼到 192×208：左右置中，最下緣對齊 feet_y。"""
    w, h = max(1, round(im.width * scale)), max(1, round(im.height * scale))
    # 預乘 alpha 再縮放，透明邊不會混進黑邊。
    small = im.convert('RGBa').resize((w, h), Image.LANCZOS).convert('RGBA')
    canvas = Image.new('RGBA', (CELL_W, CELL_H), (0, 0, 0, 0))
    canvas.alpha_composite(small, ((CELL_W - w) // 2, feet_y - h))
    return canvas


def rows_of(f):
    """各列的幀，照 docs/MAKING_A_PET.md 的列與幀數。"""
    a, b = f['run-a'], f['run-b']
    right = [a, a, b, b, a, a, b, b]
    return {
        'idle': [f['idle-a']] * 3 + [f['idle-b']] + [f['idle-a']] * 2,
        'running-right': right,
        'running-left': [ImageOps.mirror(x) for x in right],
        'waving': [f['wave'], f['idle-a'], f['wave'], f['idle-a']],
        'jumping': [f['idle-a'], f['jump'], f['jump'], f['jump'], f['idle-a']],
        'failed': [f['failed']] * 8,
        'waiting': [f['waiting']] * 6,
        'running': [f['working']] * 6,
        'review': [f['review']] * 6,
        'look': [f['idle-a']] * 16,
    }


def preview(sheet_path, out, rows=3):
    """圖集的前幾列，鋪在淺色棋盤格上，畫出格線。"""
    sheet = Image.open(sheet_path).convert('RGBA')
    top = sheet.crop((0, 0, sheet.width, CELL_H * rows))
    board = Image.new('RGBA', top.size, (236, 238, 242, 255))
    tile = Image.new('RGBA', (16, 16), (222, 225, 231, 255))
    for y in range(0, top.height, 16):
        for x in range(0, top.width, 16):
            if (x // 16 + y // 16) % 2:
                board.paste(tile, (x, y))
    board.alpha_composite(top)
    px = board.load()
    for x in range(0, top.width, CELL_W):
        for y in range(top.height):
            px[x, y] = (180, 186, 196, 255)
    for y in range(0, top.height, CELL_H):
        for x in range(top.width):
            px[x, y] = (180, 186, 196, 255)
    board.convert('RGB').save(out)


def main():
    p = argparse.ArgumentParser(description='把 3×3 姿勢表排成 Wakuwaku 的寵物圖集')
    p.add_argument('sheet')
    p.add_argument('out')
    p.add_argument('--run-b')
    p.add_argument('--id')
    p.add_argument('--name')
    p.add_argument('--fit', choices=['same', 'each'], default='same')
    p.add_argument('--alpha', type=int, default=8, help='alpha 小於這個值的點當成透明（預設 8）')
    p.add_argument('--preview', help='另存圖集前三列的預覽 PNG')
    args = p.parse_args()

    pet_id = args.id or Path(args.sheet).stem
    name = args.name or pet_id
    cells = cut(clean(Image.open(args.sheet).convert('RGBA'), args.alpha))
    poses = dict(zip(POSES, cells))
    if args.run_b:
        run_b = clean(Image.open(args.run_b).convert('RGBA'), args.alpha)
        poses['run-b'] = run_b.crop(run_b.getbbox())
    else:
        poses['run-b'] = poses['run-a']

    same = min(scale_of(im) for im in poses.values())
    frames = {k: place(im, same if args.fit == 'same' else scale_of(im), FEET_Y.get(k, FEET_Y_DEFAULT)) for k, im in poses.items()}
    for k, im in poses.items():
        s = same if args.fit == 'same' else scale_of(im)
        print(f'{k:8} {im.width}×{im.height} → {round(im.width * s)}×{round(im.height * s)}（×{s:.3f}）')

    out = Path(args.out)
    for row, images in rows_of(frames).items():
        folder = out / row
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir(parents=True)
        for i, im in enumerate(images):
            im.save(folder / f'{i:02}.png')

    pet = ROOT / 'data' / 'pets' / pet_id
    pet.mkdir(parents=True, exist_ok=True)
    make_sheet.main(out, pet / 'spritesheet.webp')
    meta = {
        'id': pet_id,
        'displayName': name,
        'description': DESCRIPTION,
        'spritesheetPath': 'spritesheet.webp',
        'spriteVersionNumber': 2,
        'kind': 'person',
    }
    (pet / 'pet.json').write_text(json.dumps(meta, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{pet / "pet.json"}')
    if args.preview:
        preview(pet / 'spritesheet.webp', args.preview)
        print(f'預覽：{args.preview}')


if __name__ == '__main__':
    main()
