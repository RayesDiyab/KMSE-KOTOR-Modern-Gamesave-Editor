"""The save editor's icon files: the art (a rendered holocron and datapad, cropped to its content and centred in a
square with a small margin, kept as art.png) with KMSE under it in the logo's letters (kmse-letters.png, made by
letters.py), the letters over the art's bottom edge (2026-11-09, the maintainer). Then:

- icon.png, 1024 px: the master (macOS builds its app icon from it);
- ../save-editor-icon.png, 512 px: the window's icon and the Linux AppImage's;
- ../save-editor.ico, 16 to 256 px: the program file. At 16 to 32 px the word cannot be read, so those sizes
  are the art alone; from 40 px the art with the word.

    python make_icons.py rendered.png     (a new render: art.png is made from it first)
    python make_icons.py                  (art.png as it is)
"""
import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
N = 1024
ART_WIDTH = 0.86      # the art's width, of the icon's
WORD_WIDTH = 0.96     # the word at most this wide
OVERLAP = 0.30        # the art's bottom reaches this far down the letters (of their height)
PAD = 10              # the letters' file is padded by 10 px (build_brand_image.py)

if len(sys.argv) > 1:
    im = Image.open(sys.argv[1]).convert("RGBA")
    c = im.crop(im.getbbox())
    side = round(max(c.size) * 1.04)
    sq = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    sq.alpha_composite(c, ((side - c.width) // 2, (side - c.height) // 2))
    sq.resize((N, N), Image.LANCZOS).save(HERE / "art.png")

art = Image.open(HERE / "art.png").convert("RGBA")
art = art.crop(art.getbbox())
word = Image.open(HERE / "kmse-letters.png").convert("RGBA")
pad = PAD
if word.width > N * WORD_WIDTH:
    f = N * WORD_WIDTH / word.width
    word = word.resize((round(word.width * f), round(word.height * f)), Image.LANCZOS)
    pad = round(PAD * f)
w = round(N * ART_WIDTH)
art = art.resize((w, round(art.height * w / art.width)), Image.LANCZOS)

icon = Image.new("RGBA", (N, N), (0, 0, 0, 0))
letters_top = N - word.height + pad // 2
art_bottom = letters_top + round((word.height - 2 * pad) * OVERLAP)
icon.alpha_composite(art, ((N - art.width) // 2, max(0, art_bottom - art.height)))
icon.alpha_composite(word, ((N - word.width) // 2, N - word.height))
icon.save(HERE / "icon.png")
icon.resize((512, 512), Image.LANCZOS).save(HERE.parent / "save-editor-icon.png", optimize=True)

# the art alone, square, for the sizes the word cannot be read at
plain = Image.open(HERE / "art.png").convert("RGBA")
sizes = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
frames = [(plain if s <= 32 else icon).resize((s, s), Image.LANCZOS) for s in sizes]
frames[-1].save(HERE.parent / "save-editor.ico", sizes=[(s, s) for s in sizes], append_images=frames[:-1])
print("icon.png, ../save-editor-icon.png, ../save-editor.ico")
