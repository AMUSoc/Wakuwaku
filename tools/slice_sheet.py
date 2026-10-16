"""把 3×3 的 Q 版姿勢表切成 Wakuwaku 寵物的各列幀，排成圖集（docs/MAKING_A_PET.md）。

用法：
  python tools/slice_sheet.py <姿勢表.png> <輸出資料夾> [--run-a 檔案] [--run-b 檔案] [--id 寵物id]
                              [--name 顯示名] [--description 說明] [--color #rrggbb] [--match-eyes]
                              [--eyes-preview 對照.png]
                              [--fit same|each | --height N] [--preview 預覽.png]
                              [--crop-preview 裁切預覽.png] [--look 視線表.png] [--breathe]
                              [--look-preview 視線預覽.png]

姿勢表：3×3，透明背景，格子之間是透明區域，由左到右、由上到下是
  idle-a、idle-b、wave、jump、failed、waiting、working、review、run-a
--run-b：第二張跑步幀（一張圖，透明背景）；沒有就全部用 run-a。它和姿勢表通常畫得不一樣大，
  所以用自己的比例：縮放後和 run-a 一樣高；腳底、置中和 run-a 相同。
--run-a：用另外畫的一張取代姿勢表第 9 格的跑步幀。這時 run-a 和 run-b 用同一個比例（兩張是一起
  畫的）：run-a 縮放後和姿勢表第 9 格一樣高，run-b 照同一比例。
--description：寫進 pet.json 的說明；不給就沿用已有 pet.json 的說明。
--color：寫進 pet.json 的角色色（#rrggbb：桌面上的泡泡、確認面板的外框）；不給就沿用已有的，
  沒有就不寫（用它 agent 的預設色）。
--match-eyes：run-a、run-b 的眼睛顏色對齊到 idle-a（另外畫的跑步幀常和姿勢表的眼睛深淺不一）。
  眼睛的找法：人的上面 45% 裡，色相在紫色附近（225–290°）、有點飽和的連通區塊中，正上方深色
  睫毛最多的那塊（idle-a 正面，取兩塊）。目標色是 idle-a 虹膜的飽和度、亮度中位數；跑步幀眼睛
  範圍裡的紫色像素（不含白色反光和黑色睫毛），飽和度、亮度按比例縮放到中位數和目標一樣，深淺
  層次留著，色相不動，其他像素不碰。找到的範圍和前後的數值會印出來；--eyes-preview 另存前後的
  眼睛局部放大對照。

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
  --look：4×4 的視線方向表，由左到右、由上到下是 look 的 index 0–15（0 看正上，順時針每
     22.5°），切法同上，依序放進 look/ 的 16 張（沒有就 16 張都是 idle-a）。視線表的人和姿勢表的
     畫得不一樣大，所以它用自己的比例：16 格高度的中位數縮成和 idle-a 一樣高；腳底、置中同上。
  --breathe：單張圖重複的列都加上程式微動（沒有就是同一張重複；idle 是 A A A B A A），腳底都固定
     在 y=204：
       idle      idle-a 呼吸，6 幀垂直縮放 1.000、1.008、1.015、1.008、1.000、0.995，index 3 換成
                 idle-b 眨眼
       waiting、review   同一條呼吸曲線
       running   working 打字時身體微晃：1.000、1.006、1.012、1.006、1.000、0.996
       failed    左右各 1–2 px 的輕微顫抖交替，8 幀：−1、+1、−2、+2、−1、+1、−2、+2
  3. 排成各列資料夾（<輸出資料夾>/idle/00.png …），running-left 是 running-right 的水平鏡像。
  4. 呼叫 make_sheet.py 產出 data/pets/<id>/spritesheet.webp，並寫 pet.json。
需要 Pillow 和 NumPy：pip install pillow numpy
"""
import argparse
import json
import re
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


def split(ink, n, what):
    """切成 n 段。ink 是一條投影上每一行（或欄）的墨量（alpha 的和）。
    有東西的區段多於 n 段時（小碎點），併到最近的大段；少於 n 段時（相鄰兩格的髮梢、鞋尖
    淡淡地碰在一起，中間沒有全透明的行），在每條預期的格線附近找墨最少的那一行切開。"""
    filled = ink > 0
    parts = runs(filled)
    if len(parts) < n:
        return split_by_ink(ink, n, what)
    while len(parts) > n:
        # 最小的一段併進相鄰、間隔較近的那段。
        i = min(range(len(parts)), key=lambda k: parts[k][1] - parts[k][0])
        left = parts[i][0] - parts[i - 1][1] if i > 0 else None
        right = parts[i + 1][0] - parts[i][1] if i + 1 < len(parts) else None
        j = i - 1 if right is None or (left is not None and left <= right) else i + 1
        a, b = sorted((i, j))
        parts[a:b + 1] = [(parts[a][0], parts[b][1])]
    if len(parts) != n:
        sys.exit(f'{what}：透明區域只分出 {len(parts)} 段，要 {n} 段；格子之間要留透明的間隔')
    return parts


def split_by_ink(ink, n, what):
    """在 n−1 條預期的格線（等分點）前後各 1/(2n) 的範圍裡，挑墨最少的一行切開。"""
    size = len(ink)
    reach = size // (2 * n)
    cuts = []
    for k in range(1, n):
        at = k * size // n
        lo, hi = max(0, at - reach), min(size, at + reach)
        cuts.append(lo + int(np.argmin(ink[lo:hi])))
    print(f'{what}：格子之間沒有全透明的間隔，在墨最少的地方切開：{cuts}（那幾行的墨量 {[int(ink[c]) for c in cuts]}）')
    edges = [0] + cuts + [size]
    parts = []
    for a, b in zip(edges, edges[1:]):
        on = np.nonzero(ink[a:b] > 0)[0]
        if not len(on):
            sys.exit(f'{what}：第 {len(parts) + 1} 段是空的')
        parts.append((a + int(on[0]), a + int(on[-1]) + 1))
    return parts


def body_box(cell, solid=64):
    """格子裡人的範圍：alpha ≥ solid 的部分的外框。"""
    a = np.array(cell.getchannel('A'))
    ys, xs = np.nonzero(a >= solid)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def drop_strays(cell, solid=64, margin=2):
    """切開時跨過格線的鄰格淡邊（髮梢、鞋尖）清掉：人的範圍（body_box）外超過 margin 的
    淡點（alpha < solid）。人自己的反鋸齒邊在範圍外 margin 以內，留著；腳底對齊才不會被
    鄰格的淡邊帶偏。"""
    x0, y0, x1, y1 = body_box(cell, solid)
    a = np.array(cell.getchannel('A'))
    keep = np.zeros(a.shape, dtype=bool)
    keep[max(0, y0 - margin):y1 + margin, max(0, x0 - margin):x1 + margin] = True
    a[~keep & (a < solid)] = 0
    out = cell.copy()
    out.putalpha(Image.fromarray(a))
    return out


def cut(sheet, n=3):
    """n×n 的表切成 n² 格（由左到右、由上到下），各自清掉鄰格的淡邊、裁到不透明部分的邊。"""
    a = np.array(sheet.getchannel('A'), dtype=np.int64)
    cells = []
    for y0, y1 in split(a.sum(axis=1), n, '列'):
        band = a[y0:y1]
        for x0, x1 in split(band.sum(axis=0), n, f'第 {len(cells) // n + 1} 列的欄'):
            cell = drop_strays(sheet.crop((x0, y0, x1, y1)))
            cells.append(cell.crop(cell.getbbox()))
    return cells


# 眼睛：紫色的色相範圍（度）、算虹膜的飽和度下限、上面幾成算頭。
EYE_HUE = (225, 290)
EYE_HEAD = 0.45


def hsv_of(im):
    """H（度）、S、V（0–1）三個陣列。"""
    h = np.array(im.convert('RGB').convert('HSV'), dtype=float) / 255.0
    return h[..., 0] * 360, h[..., 1], h[..., 2]


def components(mask):
    """8 連通的區塊編號（不用 scipy）。"""
    lab = np.zeros(mask.shape, dtype=np.int32)
    height, width = mask.shape
    n = 0
    for y0, x0 in zip(*np.nonzero(mask)):
        if lab[y0, x0]:
            continue
        n += 1
        lab[y0, x0] = n
        todo = [(y0, x0)]
        while todo:
            y, x = todo.pop()
            for yy in (y - 1, y, y + 1):
                for xx in (x - 1, x, x + 1):
                    if 0 <= yy < height and 0 <= xx < width and mask[yy, xx] and not lab[yy, xx]:
                        lab[yy, xx] = n
                        todo.append((yy, xx))
    return lab, n


def find_eyes(im, count):
    """人的上面 EYE_HEAD 裡，紫色、有點飽和的區塊中，正上方睫毛（很暗的點）最多的 count 塊的範圍。"""
    hue, sat, val = hsv_of(im)
    a = np.array(im.getchannel('A'))
    top = int(a.shape[0] * EYE_HEAD)
    iris = (a > 200) & (hue > EYE_HUE[0]) & (hue < EYE_HUE[1]) & (sat >= 0.2) & (val > 0.35) & (val < 0.97)
    iris[top:] = False
    lash = (a > 200) & (val < 0.18)
    lab, n = components(iris)
    found = []
    for k in range(1, n + 1):
        ys, xs = np.nonzero(lab == k)
        if len(ys) < 12:
            continue
        x0, y0, x1, y1 = int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1
        reach = max(3, (y1 - y0) // 4)
        found.append((int(lash[max(0, y0 - reach):y0 + 3, x0:x1].sum()), (x0, y0, x1, y1)))
    found.sort(reverse=True)
    if not found:
        sys.exit('找不到眼睛：上面沒有紫色、上方有睫毛的區塊')
    # 第二塊得分不到第一塊的一半就不是眼睛（側臉只看得到一隻）。
    best = found[0][0]
    return [box for score, box in found[:count] if score >= best / 2]


def eye_pixels(im, boxes, margin=3):
    """眼睛範圍（各往外 margin）裡要調的像素：紫色、不是白色反光（S < 0.1）也不是睫毛（V < 0.15）。"""
    hue, sat, val = hsv_of(im)
    a = np.array(im.getchannel('A'))
    inside = np.zeros(a.shape, dtype=bool)
    for x0, y0, x1, y1 in boxes:
        inside[max(0, y0 - margin):y1 + margin, max(0, x0 - margin):x1 + margin] = True
    return inside & (a > 0) & (hue > EYE_HUE[0]) & (hue < EYE_HUE[1]) & (sat >= 0.1) & (val >= 0.15) & (val < 0.97)


def eye_color(im, boxes):
    """眼睛範圍裡要調的像素的飽和度、亮度、色相中位數。"""
    hue, sat, val = hsv_of(im)
    m = eye_pixels(im, boxes)
    return float(np.median(sat[m])), float(np.median(val[m])), float(np.median(hue[m]))


def match_eyes(im, boxes, target):
    """眼睛的紫色像素：飽和度、亮度各乘一個倍數，中位數對齊到 target；色相不動，其他像素原樣。"""
    m = eye_pixels(im, boxes)
    s_now, v_now, _ = eye_color(im, boxes)
    hsv = np.array(im.convert('RGB').convert('HSV'), dtype=float)
    hsv[..., 1][m] = np.clip(hsv[..., 1][m] * target[0] / s_now, 0, 255)
    hsv[..., 2][m] = np.clip(hsv[..., 2][m] * target[1] / v_now, 0, 255)
    rgb = np.array(Image.fromarray(hsv.round().astype(np.uint8), 'HSV').convert('RGB'))
    out = np.array(im)
    out[..., :3][m] = rgb[m]
    return Image.fromarray(out, 'RGBA'), int(m.sum())


def eyes_preview(rows, out, height=150):
    """眼睛局部放大對照：每一列 [標題, 圖, 範圍] 的幾格並排；範圍各往外留一點，放大到同一個高度。"""
    tiles = []
    for row in rows:
        line = []
        for im, boxes in row:
            x0 = min(b[0] for b in boxes); y0 = min(b[1] for b in boxes)
            x1 = max(b[2] for b in boxes); y1 = max(b[3] for b in boxes)
            pad = max(8, (y1 - y0) // 2)
            crop = im.crop((x0 - pad, y0 - pad, x1 + pad, y1 + pad))
            board = Image.new('RGBA', crop.size, (255, 255, 255, 255))
            board.alpha_composite(crop)
            z = height / crop.height
            line.append(board.convert('RGB').resize((max(1, round(crop.width * z)), height), Image.NEAREST if z >= 2 else Image.LANCZOS))
        tiles.append(line)
    gap = 12
    width = max(sum(t.width for t in line) + gap * (len(line) + 1) for line in tiles)
    sheet = Image.new('RGB', (width, len(tiles) * (height + gap) + gap), (255, 255, 255))
    for i, line in enumerate(tiles):
        x = gap
        for t in line:
            sheet.paste(t, (x, gap + i * (height + gap)))
            x += t.width + gap
    sheet.save(out)


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


# 呼吸：idle（和 waiting、review）6 幀各自的垂直縮放；idle 的 index 3 是眨眼。
BREATH = [1.000, 1.008, 1.015, 1.008, 1.000, 0.995]
BLINK_AT = 3
# 打字時身體微晃：running（working）6 幀的垂直縮放。
TYPING = [1.000, 1.006, 1.012, 1.006, 1.000, 0.996]
# 顫抖：failed 8 幀各自左右移幾個像素。
SHAKE = [-1, 1, -2, 2, -1, 1, -2, 2]


def stretch(cell, f, feet_y=FEET_Y_DEFAULT):
    """格子垂直縮放 f 倍，y=feet_y 那一行不動（腳底固定）。"""
    if f == 1:
        return cell
    # 輸出的 y 取自來源的 feet_y + (y − feet_y) / f；預乘 alpha 再取樣。
    out = cell.convert('RGBa').transform(cell.size, Image.AFFINE, (1, 0, 0, 0, 1 / f, feet_y - feet_y / f), resample=Image.BICUBIC).convert('RGBA')
    # 腳底以下本來就是空的：取樣滲下去的淡邊清掉，腳底線不變。
    out.paste((0, 0, 0, 0), (0, feet_y, out.width, out.height))
    return out


def shift(cell, dx):
    """格子左右移 dx 個像素（腳底不動）；移出格子的部分裁掉。"""
    out = Image.new('RGBA', cell.size, (0, 0, 0, 0))
    out.paste(cell, (dx, 0))
    return out


def rows_of(f, look=None, breathe=False):
    """各列的幀，照 docs/MAKING_A_PET.md 的列與幀數。"""
    a, b = f['run-a'], f['run-b']
    right = [a, a, b, b, a, a, b, b]
    if breathe:
        idle = [stretch(f['idle-b'] if i == BLINK_AT else f['idle-a'], k) for i, k in enumerate(BREATH)]
        failed = [shift(f['failed'], dx) for dx in SHAKE]
        waiting = [stretch(f['waiting'], k) for k in BREATH]
        running = [stretch(f['working'], k) for k in TYPING]
        review = [stretch(f['review'], k) for k in BREATH]
    else:
        idle = [f['idle-a']] * 3 + [f['idle-b']] + [f['idle-a']] * 2
        failed, waiting, running, review = [f['failed']] * 8, [f['waiting']] * 6, [f['working']] * 6, [f['review']] * 6
    return {
        'idle': idle,
        'running-right': right,
        'running-left': [ImageOps.mirror(x) for x in right],
        'waving': [f['wave'], f['idle-a'], f['wave'], f['idle-a']],
        'jumping': [f['idle-a'], f['jump'], f['jump'], f['jump'], f['idle-a']],
        'failed': failed,
        'waiting': waiting,
        'running': running,
        'review': review,
        'look': look or [f['idle-a']] * 16,
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


def crop_preview(poses, scales, out, names=('jump', 'run-a', 'run-b'), zoom=2):
    """幾格裁切後的樣子：上面是圖集裡的格子，下面是縮放後的整張，被裁掉的部分蓋上紅色。
    上面那排的格子之間畫一條 y=204 的腳底線。"""
    gap = 24
    parts = []
    for name in names:
        cell, cut = place(poses[name], scales[name], FEET_Y.get(name, FEET_Y_DEFAULT), by_mass=True)
        small = scaled(poses[name], scales[name])
        width = max(CELL_W, small.width)
        block = Image.new('RGBA', (width, CELL_H + gap + small.height), (255, 255, 255, 0))
        top = board_of((CELL_W, CELL_H))
        top.alpha_composite(cell)
        top.paste((60, 140, 235, 255), (0, FEET_Y_DEFAULT, CELL_W, FEET_Y_DEFAULT + 1))
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


def preview(sheet_path, out, rows=3, first=0):
    """圖集的幾列（從第 first 列起 rows 列），鋪在淺色棋盤格上，畫出格線。"""
    sheet = Image.open(sheet_path).convert('RGBA')
    top = sheet.crop((0, CELL_H * first, sheet.width, CELL_H * (first + rows)))
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
    p.add_argument('--run-a', help='另外畫的跑步幀，取代姿勢表第 9 格')
    p.add_argument('--run-b')
    p.add_argument('--description', help='pet.json 的說明（不給就沿用已有的）')
    p.add_argument('--color', help='pet.json 的角色色 #rrggbb（不給就沿用已有的）')
    p.add_argument('--match-eyes', action='store_true', help='run-a、run-b 的眼睛顏色對齊到 idle-a')
    p.add_argument('--eyes-preview', help='另存 --match-eyes 前後的眼睛局部放大對照')
    p.add_argument('--id')
    p.add_argument('--name')
    p.add_argument('--fit', choices=['same', 'each'], default='same')
    p.add_argument('--height', type=int, help='9 格同一比例，idle-a 高 N；超出 192 寬的左右裁掉')
    p.add_argument('--alpha', type=int, default=8, help='alpha 小於這個值的點當成透明（預設 8）')
    p.add_argument('--preview', help='另存圖集前三列的預覽 PNG')
    p.add_argument('--crop-preview', help='另存 jump 和 run-a 裁切後的樣子')
    p.add_argument('--look', help='4×4 的視線方向表（look 的 index 0–15）')
    p.add_argument('--breathe', action='store_true', help='單張圖重複的列加上微動：idle、waiting、review 呼吸，running 微晃，failed 顫抖')
    p.add_argument('--look-preview', help='另存圖集 look 兩列（第 9、10 列）的預覽 PNG')
    args = p.parse_args()

    # Before anything is written.
    if args.color is not None and not re.fullmatch(r'#[0-9A-Fa-f]{6}', args.color):
        sys.exit(f'--color 要是 #rrggbb：{args.color}')
    pet_id = args.id or Path(args.sheet).stem
    name = args.name or pet_id
    cells = cut(clean(Image.open(args.sheet).convert('RGBA'), args.alpha))
    poses = dict(zip(POSES, cells))
    if args.height:
        same = args.height / poses['idle-a'].height
    else:
        same = min(scale_of(im) for im in poses.values())
    scales = {k: scale_of(im) if args.fit == 'each' and not args.height else same for k, im in poses.items()}

    def drawn(path):
        im = drop_strays(clean(Image.open(path).convert('RGBA'), args.alpha))
        return im.crop(im.getbbox())

    if args.run_a:
        # 另外畫的一張：縮放後和姿勢表第 9 格一樣高；run-b 和它一起畫的，用同一比例。
        sheet_run = poses['run-a']
        poses['run-a'] = drawn(args.run_a)
        scales['run-a'] = scales['run-a'] * sheet_run.height / poses['run-a'].height
    if args.run_b:
        poses['run-b'] = drawn(args.run_b)
        # 和 run-a 一起畫的：同一比例；否則縮放後和 run-a 一樣高。
        scales['run-b'] = scales['run-a'] if args.run_a else scales['run-a'] * poses['run-a'].height / poses['run-b'].height
    else:
        poses['run-b'] = poses['run-a']
        scales['run-b'] = scales['run-a']

    if args.match_eyes:
        idle_eyes = find_eyes(poses['idle-a'], 2)
        target = eye_color(poses['idle-a'], idle_eyes)
        print(f'眼睛目標色（idle-a，{len(idle_eyes)} 隻 {idle_eyes}）：S {target[0]:.2f}、V {target[1]:.2f}、H {target[2]:.0f}°')
        rows = [[(poses['idle-a'], idle_eyes)]]
        for k in ['run-a', 'run-b'] if args.run_b else ['run-a']:
            eyes = find_eyes(poses[k], 2)
            before = eye_color(poses[k], eyes)
            was = poses[k]
            poses[k], changed = match_eyes(was, eyes, target)
            after = eye_color(poses[k], eyes)
            print(f'{k} 眼睛 {eyes}：{changed} px，S {before[0]:.2f} → {after[0]:.2f}、V {before[1]:.2f} → {after[1]:.2f}、H {before[2]:.0f}° → {after[2]:.0f}°')
            rows.append([(was, eyes), (poses[k], eyes)])
        if args.eyes_preview:
            eyes_preview(rows, args.eyes_preview)
            print(f'眼睛對照：{args.eyes_preview}')

    frames = {}
    for k, im in poses.items():
        s = scales[k]
        frames[k], trimmed = place(im, s, FEET_Y.get(k, FEET_Y_DEFAULT), by_mass=bool(args.height))
        lost = trimmed['left'] + trimmed['right'] + trimmed['top']
        said = f'裁掉 左 {trimmed["left"]}、右 {trimmed["right"]}、上 {trimmed["top"]} px' + ('  ! 超過 40 px' if lost > 40 else '')
        print(f'{k:8} {im.width}×{im.height} → {round(im.width * s)}×{round(im.height * s)}（×{s:.3f}）  {said}')

    look = None
    if args.look:
        cells = cut(clean(Image.open(args.look).convert('RGBA'), args.alpha), 4)
        middle = float(np.median([im.height for im in cells]))
        tall = poses['idle-a'].height * same
        s = tall / middle
        print(f'look：16 格高 {min(im.height for im in cells)}–{max(im.height for im in cells)}，中位數 {middle:g} → {tall:.0f}（×{s:.3f}）')
        look = []
        for i, im in enumerate(cells):
            cell, trimmed = place(im, s, FEET_Y_DEFAULT, by_mass=bool(args.height))
            look.append(cell)
            lost = trimmed['left'] + trimmed['right'] + trimmed['top']
            if lost:
                print(f'look {i:2}  裁掉 左 {trimmed["left"]}、右 {trimmed["right"]}、上 {trimmed["top"]} px' + ('  ! 超過 40 px' if lost > 40 else ''))

    out = Path(args.out)
    for row, images in rows_of(frames, look, args.breathe).items():
        folder = out / row
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir(parents=True)
        for i, im in enumerate(images):
            im.save(folder / f'{i:02}.png')

    pet = ROOT / 'data' / 'pets' / pet_id
    pet.mkdir(parents=True, exist_ok=True)
    make_sheet.main(out, pet / 'spritesheet.webp')
    try:
        before = json.loads((pet / 'pet.json').read_text(encoding='utf-8'))
    except (OSError, ValueError):
        before = {}
    description = args.description if args.description is not None else before.get('description', '')
    color = args.color if args.color is not None else before.get('color')
    meta = {
        'id': pet_id,
        'displayName': name,
        'description': description,
        'spritesheetPath': 'spritesheet.webp',
        'spriteVersionNumber': 2,
        'kind': 'person',
    }
    if color:
        meta['color'] = color
    (pet / 'pet.json').write_text(json.dumps(meta, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{pet / "pet.json"}')
    if args.preview:
        preview(pet / 'spritesheet.webp', args.preview)
        print(f'預覽：{args.preview}')
    if args.look_preview:
        preview(pet / 'spritesheet.webp', args.look_preview, rows=2, first=make_sheet.LOOK_ROW)
        print(f'視線預覽：{args.look_preview}')
    if args.crop_preview:
        crop_preview(poses, scales, args.crop_preview, names=('jump', 'run-a', 'run-b') if args.run_b else ('jump', 'run-a'))
        print(f'裁切預覽：{args.crop_preview}')


if __name__ == '__main__':
    main()
