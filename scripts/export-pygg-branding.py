#!/usr/bin/env python3
"""Export Pygg vector branding as PNG and ICO (requires cairosvg and Pillow)."""
from pathlib import Path
from io import BytesIO
import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "branding"
for size in (32, 64, 128, 256, 512):
    data = cairosvg.svg2png(url=str(BRAND / "pygg-icon.svg"), output_width=size, output_height=size)
    image = Image.open(BytesIO(data)).convert("RGBA")
    image.save(BRAND / f"pygg-icon-{size}.png", optimize=True)

icon = Image.open(BRAND / "pygg-icon-256.png").convert("RGBA")
icon.save(BRAND / "pygg.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

data = cairosvg.svg2png(url=str(BRAND / "pygg-logo.svg"), output_width=1940, output_height=540)
Image.open(BytesIO(data)).save(BRAND / "pygg-logo-full.png", optimize=True)
print("Exported Pygg PNG and ICO brand assets")
