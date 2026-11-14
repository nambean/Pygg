"""Draws the Windows installers' artwork (NSIS and WiX, apps/desktop/src-tauri/installer/).

    python scripts/installer-art.py

- sidebar.bmp: the Welcome and Finish pages' picture (MUI's 164 x 314, drawn at 2x for high-DPI screens).
- header.bmp: the other pages' corner picture (MUI's 150 x 57, at 2x).
- wix-dialog.bmp, wix-banner.bmp: the MSI's pictures (WiX's 493 x 312 Welcome and Finish background, its text on the
  white right part; the 493 x 58 banner of the other dialogs, only the mark at its right end), at 2x.
- install-100.avi, install-150.avi, install-200.avi: the animation played while files are copied (installer-hooks.nsh
  picks the one for the window's DPI). It shows what Pygg does, as the gateway really works: an agent's request
  reaches Pygg on 127.0.0.1:8741, goes to a provider; a provider out of quota answers 429 and the next one answers
  before anything reached the agent; every request lands in the ledger. Windows' Animate control (SysAnimate32)
  plays it by itself, so the installer needs no plug-in; it reads uncompressed or RLE8 AVI only, so the frames are
  8-bit with one palette and RLE8-compressed here.

Needs Pillow and the Segoe UI and Consolas fonts of Windows. The outputs are committed; run this again after changing
the drawing.
"""
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "apps" / "desktop" / "src-tauri" / "installer"
ICON = ROOT / "branding" / "pygg-icon-512.png"
FONTS = Path("C:/Windows/Fonts")

ACCENT = (11, 122, 75)        # brand.json accentLight
ACCENT_DARK = (6, 78, 48)
ACCENT_SOFT = (214, 238, 226)
INK = (17, 24, 39)            # wordmark
MUTED = (102, 112, 133)       # tagline
FACE = (240, 240, 240)        # a dialog's background (COLOR_BTNFACE), so the animation sits on the page seamlessly
WHITE = (255, 255, 255)
RED = (196, 43, 28)
AMBER = (183, 110, 0)

TAGLINE = "One Flow. Many Agents."


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size)


def save_bmp(image, path):
    image.convert("RGB").save(path, format="BMP")


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


# ---------- Welcome and Finish pages: the sidebar ----------

def sidebar(scale=2):
    w, h = 164 * scale, 314 * scale
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            t = (y / h) * 0.85 + (x / w) * 0.15
            px[x, y] = lerp(ACCENT, ACCENT_DARK, t)
    d = ImageDraw.Draw(im)
    # A faint route of dots and links under the tagline: agents in, models out.
    for i, (x, y) in enumerate([(22, 206), (52, 184), (88, 198), (124, 178), (142, 214), (104, 232), (60, 246)]):
        x, y = x * scale, y * scale
        r = 3 * scale
        d.ellipse((x - r, y - r, x + r, y + r), fill=lerp(ACCENT, WHITE, .35))
    for a, b in [((22, 206), (52, 184)), ((52, 184), (88, 198)), ((88, 198), (124, 178)), ((88, 198), (142, 214)),
                 ((88, 198), (104, 232)), ((104, 232), (60, 246))]:
        d.line([(a[0] * scale, a[1] * scale), (b[0] * scale, b[1] * scale)], fill=lerp(ACCENT, WHITE, .2), width=scale)
    # The mark on a white tile.
    tile = 64 * scale
    tx, ty = (w - tile) // 2, 34 * scale
    d.rounded_rectangle((tx, ty, tx + tile, ty + tile), radius=14 * scale, fill=WHITE)
    mark = Image.open(ICON).convert("RGBA").resize((tile - 16 * scale, tile - 16 * scale), Image.LANCZOS)
    im.paste(mark, (tx + 8 * scale, ty + 8 * scale), mark)
    title = font("seguisb.ttf", 26 * scale)
    d.text((w // 2, ty + tile + 18 * scale), "Pygg", font=title, fill=WHITE, anchor="mt")
    line = font("segoeui.ttf", 10 * scale)
    d.text((w // 2, ty + tile + 54 * scale), TAGLINE, font=line, fill=lerp(WHITE, ACCENT, .15), anchor="mt")
    small = font("segoeui.ttf", 8 * scale)
    for i, text in enumerate(["A local gateway for", "Claude Code, Codex, OpenCode"]):
        d.text((w // 2, h - (44 - i * 13) * scale), text, font=small, fill=lerp(WHITE, ACCENT, .3), anchor="mt")
    return im


# ---------- Inner pages: the header picture ----------

def header(scale=2):
    w, h = 150 * scale, 57 * scale
    im = Image.new("RGB", (w, h), WHITE)
    d = ImageDraw.Draw(im)
    size = 34 * scale
    mark = Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)
    x = w - size - 10 * scale
    im.paste(mark, (x, (h - size) // 2), mark)
    d.text((x - 8 * scale, h // 2 - 7 * scale), "Pygg", font=font("seguisb.ttf", 14 * scale), fill=INK, anchor="rm")
    d.text((x - 8 * scale, h // 2 + 9 * scale), "Local AI gateway", font=font("segoeui.ttf", 8 * scale), fill=MUTED, anchor="rm")
    return im


# ---------- The MSI (WiX): the same pictures in WiX's places ----------

def wix_dialog(scale=2):
    w, h = 493 * scale, 312 * scale
    im = Image.new("RGB", (w, h), WHITE)
    side = sidebar(scale).resize((164 * scale, h), Image.LANCZOS)
    im.paste(side, (0, 0))
    return im


def wix_banner(scale=2):
    # The mark only: WiX writes the page's title and a description almost the banner's full width over it.
    w, h = 493 * scale, 58 * scale
    im = Image.new("RGB", (w, h), WHITE)
    size = 34 * scale
    mark = Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)
    im.paste(mark, (w - size - 10 * scale, (h - size) // 2), mark)
    return im


# ---------- The animation played while files are copied ----------

FPS = 10
SCENES = [  # (seconds, caption)
    (2.4, "One local gateway for every coding agent"),
    (2.8, "A provider out of quota? The next one answers."),
    (2.4, "Every request priced and kept in your ledger"),
]


def ease(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def chip(d, box, label, sub, s, accent=False, state=None):
    x0, y0, x1, y1 = box
    fill = ACCENT_SOFT if accent else WHITE
    outline = ACCENT if accent else (205, 210, 216)
    if state == "refused":
        outline, fill = RED, (252, 232, 229)
    d.rounded_rectangle(box, radius=7 * s, fill=fill, outline=outline, width=max(1, s))
    d.text((x0 + 8 * s, (y0 + y1) // 2 - (5 * s if sub else 0)), label, font=font("seguisb.ttf", 10 * s), fill=INK,
           anchor="lm")
    if sub:
        d.text((x0 + 8 * s, (y0 + y1) // 2 + 7 * s), sub, font=font("segoeui.ttf", 8 * s), fill=MUTED, anchor="lm")


def dot(d, p, s, color=ACCENT):
    r = 4 * s
    d.ellipse((p[0] - r, p[1] - r, p[0] + r, p[1] + r), fill=color)


def along(points, t):
    """A point t (0..1) of the way along a polyline."""
    lengths = [((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2) ** .5 for a, b in zip(points, points[1:])]
    goal = t * sum(lengths)
    for (a, b), n in zip(zip(points, points[1:]), lengths):
        if goal <= n or n == lengths[-1]:
            f = goal / n if n else 0
            return (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f)
        goal -= n
    return points[-1]


def frame(scene, t, s):
    """One frame at scale s; scene 0..2, t 0..1 within the scene."""
    w, h = 420 * s, 112 * s
    im = Image.new("RGB", (w, h), FACE)
    d = ImageDraw.Draw(im)
    agent = (6 * s, 22 * s, 112 * s, 58 * s)
    pygg = (162 * s, 18 * s, 258 * s, 62 * s)
    providers = [(306 * s, 6 * s, 414 * s, 30 * s), (306 * s, 28 * s + 6 * s, 414 * s, 58 * s),
                 (306 * s, 62 * s, 414 * s, 86 * s)]
    names = [("Free & cheap", None), ("Your plans", None), ("Local models", None)]
    if scene == 1:
        names = [("Provider A", "429 · out of quota" if t > .38 else None), ("Provider B", None), ("Local models", None)]
    # Links.
    a_out = (agent[2], (agent[1] + agent[3]) // 2)
    p_in = (pygg[0], (pygg[1] + pygg[3]) // 2)
    p_out = (pygg[2], p_in[1])
    d.line([a_out, p_in], fill=(190, 196, 204), width=max(1, s))
    for box in providers:
        d.line([p_out, (box[0], (box[1] + box[3]) // 2)], fill=(190, 196, 204), width=max(1, s))
    chip(d, agent, "Your agent", "Claude Code · Codex", s)
    # Pygg in the middle, with the mark.
    d.rounded_rectangle(pygg, radius=9 * s, fill=ACCENT, outline=ACCENT_DARK, width=max(1, s))
    size = 22 * s
    mark = Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)
    tile = Image.new("RGBA", (size + 4 * s, size + 4 * s), (255, 255, 255, 0))
    td = ImageDraw.Draw(tile)
    td.rounded_rectangle((0, 0, tile.width - 1, tile.height - 1), radius=5 * s, fill=WHITE)
    tile.paste(mark, (2 * s, 2 * s), mark)
    im.paste(tile, (pygg[0] + 6 * s, (pygg[1] + pygg[3] - tile.height) // 2), tile)
    d.text((pygg[0] + 36 * s, p_in[1] - 6 * s), "Pygg", font=font("seguisb.ttf", 11 * s), fill=WHITE, anchor="lm")
    d.text((pygg[0] + 36 * s, p_in[1] + 7 * s), ":8741", font=font("consola.ttf", 8 * s), fill=ACCENT_SOFT, anchor="lm")
    for i, (box, (label, problem)) in enumerate(zip(providers, names)):
        chip(d, box, label, None, s, accent=False, state="refused" if problem else None)
        if problem:
            d.text((box[2] - 6 * s, (box[1] + box[3]) // 2), "429", font=font("seguisb.ttf", 9 * s), fill=RED, anchor="rm")

    mid = lambda b: (b[0], (b[1] + b[3]) // 2)
    if scene == 0:
        # A request goes in, Pygg picks a model, the answer comes back.
        target = providers[0] if t < .5 else providers[0]
        path = [a_out, p_in, p_out, mid(target)]
        if t < .55:
            dot(d, along(path, ease(t / .55)), s)
        elif t < .95:
            dot(d, along(list(reversed(path)), ease((t - .55) / .4)), s, AMBER)
    elif scene == 1:
        first = [a_out, p_in, p_out, mid(providers[0])]
        second = [p_out, mid(providers[1])]
        if t < .38:
            dot(d, along(first, ease(t / .38)), s)
        elif t < .6:
            dot(d, along(second, ease((t - .38) / .22)), s)
        elif t < .95:
            back = [mid(providers[1]), p_out, p_in, a_out]
            dot(d, along(back, ease((t - .6) / .35)), s, AMBER)
        if t > .4:
            d.text((p_out[0] + 4 * s, p_out[1] + 12 * s), "next", font=font("segoeui.ttf", 8 * s), fill=ACCENT,
                   anchor="lm")
    else:
        # The ledger: a row per request.
        path = [a_out, p_in, p_out, mid(providers[2])]
        if t < .45:
            dot(d, along(path, ease(t / .45)), s)
        rows = min(3, int(t * 6))
        for r in range(rows):
            y = pygg[3] + (6 + r * 9) * s
            d.rounded_rectangle((pygg[0] + 8 * s, y, pygg[2] - 8 * s, y + 6 * s), radius=2 * s,
                                fill=lerp(ACCENT_SOFT, ACCENT, .25 * r))
    caption = SCENES[scene][1]
    fade = min(1.0, t / .12, (1 - t) / .12)
    d.text((w // 2, 103 * s), caption, font=font("seguisb.ttf", 10 * s), fill=lerp(FACE, INK, max(0.0, fade)),
           anchor="mm")
    return im


def frames(s):
    out = []
    for i, (seconds, _) in enumerate(SCENES):
        n = round(seconds * FPS)
        out += [frame(i, k / (n - 1), s) for k in range(n)]
    return out


def rle8(rows):
    """RLE8-encodes 8-bit rows (bottom-up), one full frame, as a DIB with BI_RLE8 expects."""
    out = bytearray()
    for row in rows:
        i, n = 0, len(row)
        while i < n:
            j = i
            while j < n and j - i < 255 and row[j] == row[i]:
                j += 1
            if j - i >= 3 or j == n:
                out += bytes((j - i, row[i]))
                i = j
                continue
            # A literal stretch until the next run of three.
            k = i
            while k < n and k - i < 255:
                if k + 2 < n and row[k] == row[k + 1] == row[k + 2]:
                    break
                k += 1
            if k - i < 3:
                for b in row[i:k]:
                    out += bytes((1, b))
            else:
                out += bytes((0, k - i)) + bytes(row[i:k])
                if (k - i) % 2:
                    out += b"\0"
            i = k
        out += b"\0\0"
    out[-2:] = b"\0\1"
    return bytes(out)


def chunk(fourcc, data):
    return fourcc + struct.pack("<I", len(data)) + data + (b"\0" if len(data) % 2 else b"")


def riff_list(kind, fourcc, data):
    return kind + struct.pack("<I", len(data) + 4) + fourcc + data


def write_avi(images, path):
    w, h = images[0].size
    strip = Image.new("RGB", (w, h * len(images)))
    for i, im in enumerate(images):
        strip.paste(im, (0, i * h))
    palette = strip.quantize(colors=256, method=Image.Quantize.MEDIANCUT)
    datas = []
    for im in images:
        q = im.quantize(palette=palette, dither=Image.Dither.NONE)
        raw = q.tobytes()
        rows = [raw[y * w:(y + 1) * w] for y in range(h - 1, -1, -1)]
        datas.append(rle8(rows))
    pal = palette.getpalette()[:768]
    pal += [0] * (768 - len(pal))
    rgbquad = b"".join(bytes((pal[i * 3 + 2], pal[i * 3 + 1], pal[i * 3], 0)) for i in range(256))
    biggest = max(len(x) for x in datas)
    bih = struct.pack("<IiiHHIIiiII", 40, w, h, 1, 8, 1, w * h, 0, 0, 256, 0) + rgbquad
    avih = struct.pack("<IIIIIIIIII4I", 1_000_000 // FPS, biggest * FPS, 0, 0x10, len(images), 0, 1, biggest, w, h,
                       0, 0, 0, 0)
    strh = struct.pack("<4s4sIHHIIIIIIIIhhhh", b"vids", b"mrle", 0, 0, 0, 0, 1, FPS, 0, len(images), biggest,
                       0xFFFFFFFF, 0, 0, 0, w, h)
    hdrl = riff_list(b"LIST", b"hdrl", chunk(b"avih", avih) + riff_list(b"LIST", b"strl",
                                                                          chunk(b"strh", strh) + chunk(b"strf", bih)))
    movi_body, index, offset = b"", b"", 4
    for data in datas:
        c = chunk(b"00dc", data)
        index += struct.pack("<4sIII", b"00dc", 0x10, offset, len(data))
        movi_body += c
        offset += len(c)
    movi = riff_list(b"LIST", b"movi", movi_body)
    body = hdrl + movi + chunk(b"idx1", index)
    path.write_bytes(b"RIFF" + struct.pack("<I", len(body) + 4) + b"AVI " + body)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    save_bmp(sidebar(), OUT / "sidebar.bmp")
    save_bmp(header(), OUT / "header.bmp")
    save_bmp(wix_dialog(), OUT / "wix-dialog.bmp")
    save_bmp(wix_banner(), OUT / "wix-banner.bmp")
    for percent, s in ((100, 1), (150, 1.5), (200, 2)):
        images = frames(2) if s == 2 else frames(1) if s == 1 else None
        if images is None:
            images = [im.resize((round(im.width * .75), round(im.height * .75)), Image.LANCZOS) for im in frames(2)]
        write_avi(images, OUT / f"install-{percent}.avi")
    # A preview strip for review: the middle of each scene, at 1x.
    preview = [frame(i, .3, 2) for i in range(len(SCENES))]
    sheet = Image.new("RGB", (preview[0].width, preview[0].height * len(preview)), FACE)
    for i, im in enumerate(preview):
        sheet.paste(im, (0, i * im.height))
    return sheet


if __name__ == "__main__":
    import sys
    sheet = main()
    if len(sys.argv) > 1:
        sheet.save(sys.argv[1])
    for p in sorted(OUT.iterdir()):
        print(f"{p.name}: {p.stat().st_size // 1024} KB")
