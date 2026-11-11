"""把 --dump 导出的多帧拼成一张图（审图只看这一张，省额度）。

用法: python tools/sheet.py frames.json out.png [缩放=0.4] [列数=2]
"""
import json, os, subprocess, sys, tempfile
from PIL import Image

src, out = sys.argv[1], sys.argv[2]
zoom = sys.argv[3] if len(sys.argv) > 3 else "0.4"
cols = int(sys.argv[4]) if len(sys.argv) > 4 else 2
tmp = tempfile.mkdtemp()
here = os.path.dirname(os.path.abspath(__file__))
subprocess.run([sys.executable, os.path.join(here, "render_frames.py"), src, tmp, zoom],
               check=True, stdout=subprocess.DEVNULL, env={**os.environ, "PYTHONIOENCODING": "utf-8"})
ims = [Image.open(os.path.join(tmp, f)) for f in sorted(os.listdir(tmp)) if f.endswith(".png")]
w, h = ims[0].size
rows = (len(ims) + cols - 1) // cols
sheet = Image.new("RGB", (w * cols + 6 * (cols - 1), h * rows + 6 * (rows - 1)), (40, 40, 40))
for k, im in enumerate(ims):
    sheet.paste(im, ((k % cols) * (w + 6), (k // cols) * (h + 6)))
sheet.save(out)
print(out, sheet.size)
