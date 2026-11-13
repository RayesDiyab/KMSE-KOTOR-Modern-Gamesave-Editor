"""Draws the macOS DMG window's background in the KMSE colours.

    python tools/dmg/background.py

Writes assets/dmg/background.png (660 x 400, the window in points) and
assets/dmg/background@2x.png (1320 x 800, for Retina screens). Needs Pillow.

The layout matches the Finder window build-macos.sh sets up: 128 pt icons, KMSE.app
centred at (170, 200) and the Applications link at (490, 200). The colours are
ui/theme.slint's dark KotOR theme. KMSE is in the crest's silver Georgia (macOS has it);
the smaller lines are in the committed KMRP Original HD Menu Small B.
"""

import math
import os
import random

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "assets", "dmg")

W, H = 660, 400           # the window, in points
S = 2                     # drawn at 2x; the 1x image is scaled down from it
APP = (170, 200)          # icon centres, in points
APPS = (490, 200)

# ui/theme.slint, dark KotOR
WINDOW = (10, 18, 32)        # window #0a1220
GLOW = (22, 36, 58)          # glow #16243a
ACCENT = (42, 198, 239)      # accent #2ac6ef
FILL_TOP = (47, 208, 245)    # fill-top #2fd0f5
TEXT = (236, 243, 248)       # text #ecf3f8
MUTED = (177, 195, 208)      # muted #b1c3d0
SOFT = (143, 163, 181)       # soft #8fa3b5
FRAME_LINE = (38, 120, 170)  # frame-line #2678aa
FRAME_DIM = (24, 72, 104)    # frame-dim #184868


def font(name, size):
    return ImageFont.truetype(os.path.join(ROOT, "fonts", name), size * S)


def p(v):
    return int(round(v * S))


def layer():
    return Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))


def glow(img, blur, strength=1.0):
    """A soft halo under img: blurred copies added together."""
    g = img.filter(ImageFilter.GaussianBlur(blur * S))
    if strength != 1.0:
        a = g.getchannel("A").point(lambda v: min(255, int(v * strength)))
        g.putalpha(a)
    return g


def radial(center, radius, color, alpha):
    """A radial wash of color that fades out at radius."""
    im = layer()
    d = ImageDraw.Draw(im)
    cx, cy = p(center[0]), p(center[1])
    steps = 60
    for i in range(steps, 0, -1):
        r = p(radius) * i / steps
        t = 1 - i / steps
        a = int(alpha * (t ** 1.6))
        d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=color + (a,))
    return im.filter(ImageFilter.GaussianBlur(6 * S))


def spaced(d, xy, text, f, fill, tracking, anchor_center=True):
    """Text with letter spacing, centred on xy."""
    # a space takes half its width, so tracked words do not drift apart
    widths = [d.textlength(c, font=f) * (0.5 if c == " " else 1) for c in text]
    total = sum(widths) + p(tracking) * (len(text) - 1)
    x = xy[0] - total / 2 if anchor_center else xy[0]
    for c, w in zip(text, widths):
        d.text((x, xy[1]), c, font=f, fill=fill, anchor="ls")
        x += w + p(tracking)


def stars(base):
    """Stars from the timeline's own field, faint, plus a few brighter ones."""
    field = Image.open(os.path.join(ROOT, "assets", "timeline", "stars.png")).convert("RGB")
    # the left half has the blue nebula; crop to the window's shape
    fw = field.width // 2 + 300
    fh = int(fw * H / W)
    field = field.crop((0, 0, fw, min(fh, field.height))).resize((W * S, H * S), Image.LANCZOS)
    base = ImageChops.screen(base, field.point(lambda v: int(v * 0.55)))
    rng = random.Random(1138)
    st = layer()
    d = ImageDraw.Draw(st)
    for _ in range(26):
        x, y = rng.uniform(0, W), rng.uniform(0, H)
        r = rng.uniform(0.6, 1.3)
        a = rng.randint(90, 200)
        d.ellipse((p(x - r), p(y - r), p(x + r), p(y + r)), fill=(200, 230, 255, a))
    base = base.convert("RGBA")
    base.alpha_composite(glow(st, 2.5, 1.5))
    base.alpha_composite(st)
    return base


def frame(im):
    """The game's menu frame: a dim line with bright corner brackets."""
    d = ImageDraw.Draw(im)
    m = 12
    x0, y0, x1, y1 = p(m), p(m), p(W - m) - 1, p(H - m) - 1
    d.rectangle((x0, y0, x1, y1), outline=FRAME_DIM + (255,), width=p(1))
    inner = 4
    d.rectangle((x0 + p(inner), y0 + p(inner), x1 - p(inner), y1 - p(inner)),
                outline=FRAME_DIM + (110,), width=max(1, p(0.5)))
    L = p(26)
    w = p(2)
    br = layer()
    bd = ImageDraw.Draw(br)
    for (cx, cy, sx, sy) in ((x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)):
        bd.line((cx, cy, cx + sx * L, cy), fill=FRAME_LINE + (255,), width=w)
        bd.line((cx, cy, cx, cy + sy * L), fill=FRAME_LINE + (255,), width=w)
        bd.rectangle((cx + sx * p(5) - p(1.5), cy + sy * p(5) - p(1.5),
                      cx + sx * p(5) + p(1.5), cy + sy * p(5) + p(1.5)), fill=ACCENT + (255,))
    im.alpha_composite(glow(br, 3, 1.2))
    im.alpha_composite(br)


def pad(im, center):
    """A holo projector pad under an icon: a lit floor and two rings."""
    cx, cy = center
    im.alpha_composite(radial((cx, cy), 92, ACCENT, 70))
    ring = layer()
    d = ImageDraw.Draw(ring)
    r = 78
    # open at the bottom, where Finder writes the icon's name
    d.arc((p(cx - r), p(cy - r), p(cx + r), p(cy + r)), 128, 52 + 360,
          fill=FRAME_LINE + (200,), width=p(1.2))
    r2 = 84
    # a dashed outer ring, like the timeline's markers
    for k in range(48):
        a0 = k * 7.5
        if k % 4 == 3 or 56 <= a0 <= 120:
            continue
        d.arc((p(cx - r2), p(cy - r2), p(cx + r2), p(cy + r2)),
              a0, a0 + 4.5, fill=ACCENT + (150,), width=p(1))
    # four ticks
    for ang in (0, 180, 270):
        t = math.radians(ang)
        x0, y0 = cx + math.cos(t) * (r2 + 3), cy + math.sin(t) * (r2 + 3)
        x1, y1 = cx + math.cos(t) * (r2 + 10), cy + math.sin(t) * (r2 + 10)
        d.line((p(x0), p(y0), p(x1), p(y1)), fill=ACCENT + (220,), width=p(1.5))
    im.alpha_composite(glow(ring, 2.5, 1.3))
    im.alpha_composite(ring)


def arrow(im):
    """One glowing arrow from the app to Applications: a beam that brightens as it goes
    and runs into a single > (2026-11-13, the maintainer; before: three separate chevrons)."""
    y = APP[1]
    x0, tip = 262, 384       # the beam's faded start and the head's point
    hw, hh = 18, 22          # the head's depth and half height
    ch = layer()
    d = ImageDraw.Draw(ch)
    # the beam, fading in from the left, meeting the head at its point
    n = 120
    for i in range(n):
        t = i / (n - 1)
        a = int(255 * t ** 1.4)
        xa = x0 + (tip - x0) * i / n
        xb = x0 + (tip - x0) * (i + 1) / n + 0.3
        d.line((p(xa), p(y), p(xb), p(y)), fill=FILL_TOP + (a,), width=p(3))
    pts = [(tip - hw, y - hh), (tip, y), (tip - hw, y + hh)]
    d.line([(p(a_), p(b_)) for a_, b_ in pts], fill=FILL_TOP + (255,), width=p(3.2),
           joint="curve")
    r = 1.6  # round the head's point and ends
    for a_, b_ in pts:
        d.ellipse((p(a_ - r), p(b_ - r), p(a_ + r), p(b_ + r)), fill=FILL_TOP + (255,))
    # a white-hot core along the beam and head, then a wide and a tight glow
    core = layer()
    cd = ImageDraw.Draw(core)
    for i in range(n):
        t = i / (n - 1)
        xa = x0 + (tip - x0) * i / n
        xb = x0 + (tip - x0) * (i + 1) / n + 0.3
        cd.line((p(xa), p(y), p(xb), p(y)), fill=(220, 248, 255, int(170 * t ** 2.2)),
                width=p(1))
    cd.line([(p(a_), p(b_)) for a_, b_ in pts], fill=(230, 250, 255, 200), width=p(1.2),
            joint="curve")
    fat = ch.filter(ImageFilter.MaxFilter(p(3) * 2 + 1))  # a thicker copy, so the halo carries
    im.alpha_composite(glow(fat, 16, 1.4))
    im.alpha_composite(glow(fat, 5, 1.2))
    im.alpha_composite(ch)
    im.alpha_composite(core)


def wordmark(im, center_x, baseline, cap):
    """KMSE in the crest's silver (assets/kmse-logo.png): Georgia, as KMRP measured its
    wordmark, tracked 0.12 cap as the KMRP and KMSE crests are, with KMRP's metal ramp
    (macos/tools/make_package_art.py): a cool silver face, brighter at the top, a warm glint
    low down, and a soft shadow (no outline, 2026-11-13, the maintainer)."""
    georgia = "/System/Library/Fonts/Supplemental/Georgia.ttf"
    f = ImageFont.truetype(georgia, p(cap / 0.69))
    text = "KMSE"
    tracking = 0.12 * cap
    mask = Image.new("L", im.size, 0)
    md = ImageDraw.Draw(mask)
    widths = [md.textlength(c, font=f) for c in text]
    x = p(center_x) - (sum(widths) + p(tracking) * (len(text) - 1)) / 2
    for c, w in zip(text, widths):
        md.text((x, p(baseline)), c, font=f, fill=255, anchor="ls")
        x += w + p(tracking)
    top, bottom = p(baseline - cap), p(baseline)
    stops = [(0.0, (238, 242, 247)), (0.45, (150, 162, 178)),
             (0.72, (206, 196, 180)), (1.0, (118, 128, 144))]
    face = layer()
    fd = ImageDraw.Draw(face)
    for y in range(top - p(2), bottom + p(4)):
        t = min(1.0, max(0.0, (y - top) / (bottom - top)))
        for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
            if t0 <= t <= t1:
                k = (t - t0) / (t1 - t0)
                fd.line([(0, y), (im.width, y)],
                        fill=tuple(round(a + (b - a) * k) for a, b in zip(c0, c1)) + (255,))
                break
    # a soft dark shadow, then the face
    shadow = Image.new("RGBA", im.size, (3, 6, 12, 0))
    shadow.putalpha(mask.filter(ImageFilter.GaussianBlur(4 * S)).point(lambda v: int(v * 0.8)))
    im.alpha_composite(shadow, (0, p(2)))
    im.paste(face, (0, 0), mask)


def titles(im):
    d = ImageDraw.Draw(im)
    label = font("KMRP-OriginalHD-MenuSmallB.otf", 10)

    # a faint light behind the letters, as the crest's glow sits behind its wordmark
    im.alpha_composite(radial((W / 2, 52), 70, (190, 210, 230), 40))
    wordmark(im, W / 2, 66, 30)

    spaced(d, (p(W / 2), p(84)), "KOTOR MODERN GAMESAVE EDITOR", label, MUTED + (255,), 3.2)

    # a thin rule with a bright centre, under the subtitle
    rule = layer()
    rd = ImageDraw.Draw(rule)
    for i in range(160):
        t_ = i / 159
        a = int(255 * (1 - abs(t_ - 0.5) * 2) ** 1.5)
        x = W / 2 - 110 + 220 * t_
        rd.line((p(x), p(96), p(x + 220 / 159 + 0.2), p(96)), fill=FRAME_LINE + (a,), width=p(1))
    im.alpha_composite(rule)

    spaced(d, (p(W / 2), p(352)), "DRAG KMSE ONTO APPLICATIONS TO INSTALL", label,
           SOFT + (255,), 3.2)


def nameplate(im, center):
    """A silver plate under an icon's name. Finder writes the names of a window with a
    background picture in black, in dark mode as well (measured on macOS 27, 2026-11-13), so
    they need a light ground; the plate is the wordmark's metal."""
    cx, cy = center[0], center[1] + 82   # the name's middle, 82 pt below the icon's centre
    w, h = 112, 20
    box = (p(cx - w / 2), p(cy - h / 2), p(cx + w / 2), p(cy + h / 2))
    mask = Image.new("L", im.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle(box, radius=p(h / 2), fill=255)
    stops = [(0.0, (236, 241, 246)), (0.5, (200, 210, 222)), (1.0, (172, 184, 199))]
    face = layer()
    fd = ImageDraw.Draw(face)
    for y in range(box[1], box[3] + 1):
        t = (y - box[1]) / (box[3] - box[1])
        for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
            if t0 <= t <= t1:
                k = (t - t0) / (t1 - t0)
                fd.line([(box[0], y), (box[2], y)],
                        fill=tuple(round(a + (b - a) * k) for a, b in zip(c0, c1)) + (255,))
                break
    halo = Image.new("RGBA", im.size, ACCENT + (0,))
    halo.putalpha(mask.filter(ImageFilter.GaussianBlur(5 * S)).point(lambda v: int(v * 0.55)))
    im.alpha_composite(halo)
    im.paste(face, (0, 0), mask)
    ImageDraw.Draw(im).rounded_rectangle(box, radius=p(h / 2), outline=FRAME_LINE + (255,),
                                         width=p(1))


def main():
    base = Image.new("RGB", (W * S, H * S), WINDOW)
    base = stars(base)
    base.alpha_composite(radial((W / 2, H * 0.55), 330, GLOW, 200))
    # darken the edges so the frame reads
    vig = layer()
    vd = ImageDraw.Draw(vig)
    vd.rectangle((0, 0, W * S, H * S), fill=(3, 6, 12, 150))
    hole = Image.new("L", (W * S, H * S), 0)
    ImageDraw.Draw(hole).ellipse((p(-60), p(-80), p(W + 60), p(H + 80)), fill=255)
    hole = hole.filter(ImageFilter.GaussianBlur(70 * S))
    vig.putalpha(ImageChops.subtract(vig.getchannel("A"), hole))
    base.alpha_composite(vig)

    pad(base, APP)
    pad(base, APPS)
    nameplate(base, APP)
    nameplate(base, APPS)
    arrow(base)
    titles(base)
    frame(base)

    os.makedirs(OUT, exist_ok=True)
    hi = base.convert("RGB")
    hi.save(os.path.join(OUT, "background@2x.png"), optimize=True)
    hi.resize((W, H), Image.LANCZOS).save(os.path.join(OUT, "background.png"), optimize=True)
    print("wrote", os.path.join(OUT, "background.png"), "and background@2x.png")


if __name__ == "__main__":
    main()
