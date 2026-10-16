# art/

美术素材和调查资料。**除了这份说明，目录里的东西都不进仓库**（见 `.gitignore`）：它们要么是别人角色的二创，要么是别人的原作参考，和代码的 MIT 许可无关，也不随安装包分发。

```
art/
├─ claude-chan/        Claude 小姐像素宠物（Codex pet v2 格式），按版本放
│  ├─ v1 … v1.3        每版的 final/（pet.json + spritesheet）、帧、预览、QA、脚本
│  └─ v1.2-review      v1.2 待机动画的检查记录
├─ ocean-maid/         蓝发海洋女仆：图标、托盘图标、表情、插画
│  ├─ v1/              素材包（打开 preview.html 看整套效果）
│  └─ export-assets.js 从三张生成图导出整套素材：node art/ocean-maid/export-assets.js <icon> <peek> <body>
├─ references/         参考图，别人的作品，只作对照
│  ├─ claude/  deepseek/  nanally/
├─ grok/               Grok 小姐的 3×3 姿势表和图集预览（进仓库，见下）
├─ astra/              Astra 的姿势表、视线表、两张跑步帧和预览（进仓库，见下）
├─ gemini/             Argon 的姿势表、视线表、两张跑步帧和预览（进仓库，见下）
└─ research/           调查资料
   └─ zipzippipe-llm-avatars/   ZipZipPipe 的 LLM 拟人角色记录（含来源链接）
```

## 署名

- Claude 小姐、DeepSeek 娘等 LLM 拟人形象的原作者是 B 站 UP 主 **ZipZipPipe**（[主页](https://space.bilibili.com/4168597)）。`claude-chan/` 是基于其 Claude 小姐形象的非官方像素二创。
- 默认宠物 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan) 的作者是 Dullsaw；`ocean-maid/` 参考了它的造型。
- `references/` 里的图版权归各自作者。

- `grok/`（**例外，进仓库**）：Grok 小姐的角色设计参考自 B 站 UP 主「沉默寡言佰佰」的 Grok 4.7 形象，是非官方 Q 版二创。`grok-sheet.png` 是姿势表，`grok-look.png` 是 4×4 的视线方向表（look 的 16 格），`grok-run-b.png` 是第二张跑步帧，`preview-grok.png` 是图集前三行的预览，`preview-grok-crop.png` 是跳和两张跑步帧裁切后的样子，`preview-grok-look.png` 是 look 两行；成品在 `data/pets/grok-chan/`，用 `python tools/slice_sheet.py art/grok/grok-sheet.png tools/grok-frames --id grok-chan --name "Grok小姐" --height 200 --look art/grok/grok-look.png --run-b art/grok/grok-run-b.png --breathe` 重新生成。

- `astra/`（**例外，进仓库**）：Astra 的角色设计参考自 B 站 UP 主「沉默寡言佰佰」的 GPT-6 Astra 形象，是非官方 Q 版二创。`astra-sheet.png` 是 3×3 姿势表，`astra-look.png` 是 4×4 视线方向表，`astra-run-a.png`、`astra-run-b.png` 是两张一起画的跑步帧（同一缩放，取代姿势表第 9 格）；它们的眼睛比姿势表浅，`--match-eyes` 把饱和度、亮度对齐到 idle-a（`preview-eyes.png`：上为 idle-a，下为调整前后）。`preview-astra.png`、`preview-astra-look.png`、`preview-astra-crop.png` 是图集的预览。成品在 `data/pets/gpt-astra/`，用 `python tools/slice_sheet.py art/astra/astra-sheet.png tools/astra-frames --run-a art/astra/astra-run-a.png --run-b art/astra/astra-run-b.png --look art/astra/astra-look.png --height 200 --breathe --match-eyes --id gpt-astra --name "Astra"` 重新生成（说明沿用 pet.json 里的）。

- `gemini/`（**例外，进仓库**）：Argon 的角色设计参考自 B 站 UP 主「沉默寡言佰佰」的 Gemini 4 Argon 形象，是非官方 Q 版二创。`gemini-sheet.png` 是 3×3 姿势表，`gemini-look.png` 是 4×4 视线方向表，`gemini-run-a.png`、`gemini-run-b.png` 是两张一起画的跑步帧。她是异色瞳（右眼红、左眼黄）：跑步帧看得到的是右眼，`--mirror-eye 28-65` 把往左跑（镜像）那几张的眼睛换成 idle-a 左眼的黄色；眼睛是红的，所以 `--eye-hue 340-15 --eye-sat 0.45`（紫色的默认值找不到她的眼睛，0.45 让肤色和发饰不被当成眼睛）。视线表最下面一行的四格脚画到了图外：那四格按头顶对齐，腿到格子里就断了，要好看得重新生成一张下面留空的视线表。`preview-eyes.png` 从上到下是 idle-a 的红眼、两张跑步帧对齐前后、两张往左跑换色前后；`preview-argon.png`、`preview-argon-look.png`、`preview-argon-crop.png` 是图集的预览。成品在 `data/pets/gemini-argon/`（角色色 `#4A8FE7`），用 `python tools/slice_sheet.py art/gemini/gemini-sheet.png tools/gemini-frames --run-a art/gemini/gemini-run-a.png --run-b art/gemini/gemini-run-b.png --look art/gemini/gemini-look.png --height 200 --breathe --match-eyes --eye-hue 340-15 --eye-sat 0.45 --mirror-eye 28-65 --id gemini-argon --name "Argon"` 重新生成（说明和颜色沿用 pet.json 里的）。

要把某套素材放进应用（比如做成默认宠物或换掉图标），需要先确认原作者的授权。
