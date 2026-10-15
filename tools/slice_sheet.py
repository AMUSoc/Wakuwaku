"""把 3×3 的 Q 版姿勢表切成 Wakuwaku 寵物的各列幀，排成圖集（docs/MAKING_A_PET.md）。

用法：
  python tools/slice_sheet.py <姿勢表.png> <輸出資料夾> [--run-b 檔案] [--id 寵物id] [--name 顯示名]
                              [--fit same|each | --height N] [--preview 預覽.png]
                              [--crop-preview 裁切預覽.png]

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
     --height N：改成 9 格同一個比例，讓 idle-a 剛好 N 高（claude-chan 是 200），不再縮小去塞進
     180×200；以人物中心（不透明部分的重心，飄開的頭髮拉不走它）左右置中，超出 192 寬的部分
     直接裁掉，超出上緣的也裁掉。每一格裁掉幾個像素都會印出來，超過 40 px 的標上「!」。
     --crop-preview 另存 jump 和 run-a 裁切後的樣子（上：圖集裡的格子；下：整張，紅色是裁掉的）。
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


def scaled(im, scale):
    w, h = max(1, round(im.width * scale)), max(1, round(im.height * scale))
    # 預乘 alpha 再縮放，透明邊不會混進黑邊。
    return im.convert('RGBa').resize((w, h), Image.LANCZOS).convert('RGBA')


def left_of(small, by_mass):
    """縮放後的圖在 192 寬的格子裡的左緣：外框置中，或重心置中。"""
    if not by_mass:
        return (CELL_W - small.width) // 2
    a = np.array(small.getchannel('A'), dtype=float)
    cx = (a.sum(axis=0) * np.arange(small.width)).sum() / a.sum()
    return round(CELL_W / 2 - cx)


def place(im, scale, feet_y, by_mass=False):
    """等比縮放後貼到 192×208：左右置中，最下緣對齊 feet_y；超出格子的裁掉。
    回傳格子，和左、右、上各裁掉幾個像素。"""
    small = scaled(im, scale)
    x, y = left_of(small, by_mass), feet_y - small.height
    canvas = Image.new('RGBA', (CELL_W, CELL_H), (0, 0, 0, 0))
    # 格子原本是全透明的：直接貼上（paste 會裁掉超出格子的部分）。
    canvas.paste(small, (x, y))
    cut = {'left': max(0, -x), 'right': max(0, x + small.width - CELL_W), 'top': max(0, -y)}
    return canvas, cut


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


def board_of(size):
    """淺色棋盤格，看得出透明的地方。"""
    board = Image.new('RGBA', size, (236, 238, 242, 255))
    tile = Image.new('RGBA', (16, 16), (222, 225, 231, 255))
    for y in range(0, size[1], 16):
        for x in range(0, size[0], 16):
            if (x // 16 + y // 16) % 2:
                board.paste(tile, (x, y))
    return board


def crop_preview(poses, scale, out, names=('jump', 'run-a'), zoom=2):
    """幾格裁切後的樣子：上面是圖集裡的格子，下面是縮放後的整張，被裁掉的部分蓋上紅色。"""
    gap = 24
    parts = []
    for name in names:
        cell, cut = place(poses[name], scale, FEET_Y.get(name, FEET_Y_DEFAULT), by_mass=True)
        small = scaled(poses[name], scale)
        width = max(CELL_W, small.width)
        block = Image.new('RGBA', (width, CELL_H + gap + small.height), (255, 255, 255, 0))
        top = board_of((CELL_W, CELL_H))
        top.alpha_composite(cell)
        block.paste(top, ((width - CELL_W) // 2, 0))
        full = board_of(small.size)
        full.alpha_composite(small)
        shade = Image.new('RGBA', small.size, (0, 0, 0, 0))
        red = (235, 60, 60, 110)
        if cut['left']:
            shade.paste(red, (0, 0, cut['left'], small.height))
        if cut['right']:
            shade.paste(red, (small.width - cut['right'], 0, small.width, small.height))
        if cut['top']:
            shade.paste(red, (0, 0, small.width, cut['top']))
        full.alpha_composite(shade)
        block.paste(full, ((width - small.width) // 2, CELL_H + gap))
        parts.append(block)
    w = sum(p.width for p in parts) + gap * (len(parts) + 1)
    h = max(p.height for p in parts) + gap * 2
    sheet = Image.new('RGBA', (w, h), (255, 255, 255, 255))
    x = gap
    for p in parts:
        sheet.alpha_composite(p, (x, gap))
        x += p.width + gap
    sheet.convert('RGB').resize((w * zoom, h * zoom), Image.NEAREST).save(out)


def preview(sheet_path, out, rows=3):
    """圖集的前幾列，鋪在淺色棋盤格上，畫出格線。"""
    sheet = Image.open(sheet_path).convert('RGBA')
    top = sheet.crop((0, 0, sheet.width, CELL_H * rows))
    board = board_of(top.size)
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
    p.add_argument('--height', type=int, help='9 格同一比例，idle-a 高 N；超出 192 寬的左右裁掉')
    p.add_argument('--alpha', type=int, default=8, help='alpha 小於這個值的點當成透明（預設 8）')
    p.add_argument('--preview', help='另存圖集前三列的預覽 PNG')
    p.add_argument('--crop-preview', help='另存 jump 和 run-a 裁切後的樣子')
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

    if args.height:
        same = args.height / poses['idle-a'].height
    else:
        same = min(scale_of(im) for im in poses.values())
    frames = {}
    for k, im in poses.items():
        s = scale_of(im) if args.fit == 'each' and not args.height else same
        frames[k], trimmed = place(im, s, FEET_Y.get(k, FEET_Y_DEFAULT), by_mass=bool(args.height))
        lost = trimmed['left'] + trimmed['right'] + trimmed['top']
        said = f'裁掉 左 {trimmed["left"]}、右 {trimmed["right"]}、上 {trimmed["top"]} px' + ('  ! 超過 40 px' if lost > 40 else '')
        print(f'{k:8} {im.width}×{im.height} → {round(im.width * s)}×{round(im.height * s)}（×{s:.3f}）  {said}')

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
    if args.crop_preview:
        crop_preview(poses, same, args.crop_preview)
        print(f'裁切預覽：{args.crop_preview}')


if __name__ == '__main__':
    main()
