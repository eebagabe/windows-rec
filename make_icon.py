"""Gera assets/icon.png e assets/icon.ico."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

S = 1024
out = Path(__file__).parent / "assets"
out.mkdir(exist_ok=True)

grad = Image.new("RGB", (S, S))
px = grad.load()
c1, c2 = (255, 59, 92), (106, 44, 255)
for y in range(S):
    for x in range(S):
        t = (x + y) / (2 * S)
        px[x, y] = tuple(int(a + (b - a) * t) for a, b in zip(c1, c2))

mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle((40, 40, S - 40, S - 40), radius=230, fill=255)
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
img.paste(grad, (0, 0), mask)

d = ImageDraw.Draw(img)
c = S // 2
d.ellipse((c - 300, c - 300, c + 300, c + 300), outline=(255, 255, 255, 255), width=56)
glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(glow).ellipse((c - 170, c - 170, c + 170, c + 170), fill=(255, 255, 255, 140))
img = Image.alpha_composite(img, glow.filter(ImageFilter.GaussianBlur(40)))
ImageDraw.Draw(img).ellipse((c - 150, c - 150, c + 150, c + 150), fill=(255, 255, 255, 255))

img.resize((256, 256), Image.LANCZOS).save(out / "icon.png")
img.save(out / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
