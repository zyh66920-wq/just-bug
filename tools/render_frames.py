"""把 --dump xxx.json 导出的帧画成 PNG（开发自查用，模拟 Windows Terminal 的真实字体）。

用法: python render_frames.py frames.json 输出目录 [缩放]
"""
import json, os, sys
from PIL import Image, ImageDraw, ImageFont

src, outdir = sys.argv[1], sys.argv[2]
zoom = float(sys.argv[3]) if len(sys.argv) > 3 else 1.0
os.makedirs(outdir, exist_ok=True)
CW, CH = 8, 16
mono = ImageFont.truetype(r"C:\Windows\Fonts\CascadiaMono.ttf", 14)
cjk = ImageFont.truetype(r"C:\Windows\Fonts\msyh.ttc", 14)


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


frames = json.load(open(src, encoding="utf-8"))
for n, f in enumerate(frames):
    w, h = f["w"], f["h"]
    img = Image.new("RGB", (w * CW, h * CH + 18), (0, 0, 0))
    d = ImageDraw.Draw(img)
    for i, (ch, fg, bg, wide) in enumerate(f["c"]):
        x, y = (i % w) * CW, (i // w) * CH + 18
        cw = CW * (2 if wide == 1 else 1)
        if wide == 2:
            continue
        fg, bg = rgb(fg), rgb(bg)
        d.rectangle([x, y, x + cw - 1, y + CH - 1], fill=bg)
        if ch == " ":
            continue
        if ch == "█":
            d.rectangle([x, y, x + CW - 1, y + CH - 1], fill=fg)
        elif ch == "▀":
            d.rectangle([x, y, x + CW - 1, y + CH // 2 - 1], fill=fg)
        elif ch == "▄":
            d.rectangle([x, y + CH // 2, x + CW - 1, y + CH - 1], fill=fg)
        else:
            font = cjk if wide == 1 else mono
            d.text((x, y - (3 if wide == 1 else 0)), ch, font=font, fill=fg)
    d.text((4, 2), f["label"], font=mono, fill=(255, 255, 0))
    if zoom != 1.0:
        img = img.resize((int(img.width * zoom), int(img.height * zoom)), Image.LANCZOS)
    img.save(os.path.join(outdir, f"{n:02d}.png"))
    print(n, f["label"])
