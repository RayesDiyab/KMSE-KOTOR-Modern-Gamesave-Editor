"""The macOS icon: icon.png on a rounded-square plate of the timeline's stars.

    python make_mac_icon.py

Writes icon-macos.png (1024 px) beside this file, the master build-macos.sh makes
AppIcon.icns from. Needs numpy and Pillow.

Since macOS 26 Finder puts an icon that is not a full rounded square on a grey-white plate.
This one fills Apple's grid (an 824 px plate, 100 px in from each edge of 1024), so it is
shown as drawn (measured on macOS 27, 2026-11-13). The maintainer chose the star plate over
a plain navy one the same day, and icon.png with KMSE under the art over art.png alone.
"""

import os

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
N = 1024
INSET = 100

WINDOW = (10, 18, 32)        # ui/theme.slint window #0a1220
GLOW = (28, 70, 120)
FRAME_LINE = (38, 120, 170)  # frame-line #2678aa
ACCENT = (42, 198, 239)      # accent #2ac6ef


def squircle_mask():
    """A superellipse (n = 5), close to Apple's continuous-corner square, drawn at 2x."""
    s = N * 2
    c = s / 2
    r = s / 2 - INSET * 2
    y, x = np.mgrid[0:s, 0:s].astype(np.float64)
    v = np.abs((x + 0.5 - c) / r) ** 5 + np.abs((y + 0.5 - c) / r) ** 5
    return Image.fromarray(((v <= 1.0) * 255).astype(np.uint8)).resize((N, N), Image.LANCZOS)


def main():
    mask = squircle_mask()

    # navy, a blue glow low in the middle, the timeline's stars over it
    plate = Image.new("RGB", (N, N), WINDOW)
    g = Image.new("L", (N, N), 0)
    ImageDraw.Draw(g).ellipse((150, 230, 874, 954), fill=255)
    plate = Image.composite(Image.new("RGB", (N, N), GLOW), plate,
                            g.filter(ImageFilter.GaussianBlur(170)))
    field = Image.open(os.path.join(ROOT, "assets", "timeline", "stars.png")).convert("RGB")
    field = field.crop((0, 0, 675, 675)).resize((N, N), Image.LANCZOS)
    plate = ImageChops.screen(plate, field.point(lambda v: int(v * 0.7))).convert("RGBA")

    # a faint sheen at the top
    top = Image.new("L", (N, N), 0)
    ImageDraw.Draw(top).rectangle((0, 0, N, 300), fill=90)
    sheen = Image.new("RGBA", (N, N), (180, 210, 240, 0))
    sheen.putalpha(ImageChops.multiply(top.filter(ImageFilter.GaussianBlur(120)), mask)
                   .point(lambda v: v // 3))
    plate.alpha_composite(sheen)

    # the icon, 700 px across, with a soft cyan light under it so its dark faces read
    art = Image.open(os.path.join(HERE, "icon.png")).convert("RGBA")
    art = art.crop(art.getbbox())
    k = 700 / max(art.size)
    art = art.resize((round(art.width * k), round(art.height * k)), Image.LANCZOS)
    ox, oy = (N - art.width) // 2, (N - art.height) // 2 + 10
    a = Image.new("L", (N, N), 0)
    a.paste(art.getchannel("A"), (ox, oy))
    halo = Image.new("RGBA", (N, N), ACCENT + (0,))
    halo.putalpha(a.filter(ImageFilter.GaussianBlur(40)).point(lambda v: int(v * 0.45)))
    plate.alpha_composite(halo)
    plate.alpha_composite(art, (ox, oy))

    # a thin cyan rim, as the game's frames
    rim = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    rim.paste(FRAME_LINE + (255,), (0, 0),
              ImageChops.subtract(mask, mask.filter(ImageFilter.MinFilter(7))))
    plate.alpha_composite(rim)

    icon = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    icon.paste(plate, (0, 0), mask)
    out = os.path.join(HERE, "icon-macos.png")
    icon.save(out, optimize=True)
    print("wrote", out)


if __name__ == "__main__":
    main()
