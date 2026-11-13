"""Draws the KMSE press kit: a cover and one page per part of the editor, 1920 x 1080.

    python tools/press_kit/build.py SHOTS [--only 01]

SHOTS is the folder of window screenshots (PNG, named as in PAGES). The pages go to
releases/v1.0/press-kit/. Needs Pillow and numpy; on macOS for Georgia.

The look is KMSE's own, not KMRP's release art: the editor's dark KotOR colours
(ui/theme.slint), the timeline's stars, the game-menu frames with bright corner brackets
the DMG window has (tools/dmg/background.py), KMSE in the crest's silver, the version in
holo cyan, and the KMRP Original HD fonts the editor itself uses. A screenshot is shown
whole, scaled down, never retouched.
"""

import argparse
import os

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "releases", "v1.0", "press-kit")
VERSION = "1.0"
W, H = 1920, 1080

# ui/theme.slint, dark KotOR; the K2 teal for the KotOR II page
WINDOW = (10, 18, 32)
GLOW = (22, 36, 58)
ACCENT = (42, 198, 239)
TEXT = (236, 243, 248)
MUTED = (177, 195, 208)
SOFT = (143, 163, 181)
FRAME_LINE = (38, 120, 170)
FRAME_DIM = (24, 72, 104)
GOLD = (226, 195, 92)
K2_ACCENT = (47, 207, 155)
K2_LINE = (31, 138, 104)

GEORGIA = "/System/Library/Fonts/Supplemental/Georgia.ttf"

# (file, kind, title, screenshot, label under the frame, caption)
PAGES = (
    ("01-cover", "cover", None, None, None, None),
    ("02-load-screen", "shot", "THE LOAD SCREEN", "01-saves",
     "SAVES  ·  KOTOR",
     "Your saves, found by themselves, with their pictures and party, as the game's Load screen shows them."),
    ("03-character", "shot", "CHARACTER", "03-character",
     "CHARACTER",
     "Name, experience, classes, alignment, attributes, Health and Force, portrait and voice."),
    ("04-abilities", "shot", "ABILITIES", "04-abilities",
     "SKILLS, FEATS AND FORCE POWERS",
     "Skills, and feats and Force powers added or taken away one at a time."),
    ("05-inventory", "shot", "INVENTORY", "05-inventory",
     "INVENTORY",
     "Credits, any item the game or a mod has, stack sizes, and what each party member wears."),
    ("06-party", "shot", "PARTY", "06-party",
     "PARTY",
     "Who is recruited, who is with you, who can be picked, and party experience."),
    ("07-timeline", "shot", "TIMELINE", "07-timeline",
     "THE STORY AS A GALAXY MAP",
     "Bands of light join the planets in the order you played them. Skip ahead, or go back."),
    ("08-quests", "shot", "QUESTS", "08-quests",
     "QUESTS",
     "Every quest, active, done and not started, with its journal entry."),
    ("09-pazaak-destinations", "shot", "PAZAAK & DESTINATIONS", "09-pazaak",
     "PAZAAK & DESTINATIONS",
     "The planets the Ebon Hawk can fly to, and your Pazaak side deck."),
    ("10-map", "shot", "MAP", "10-map",
     "THE AREA MAP",
     "The area's people, doors and containers. Move the player, travel, or spawn a creature."),
    ("11-advanced", "shot", "ADVANCED", "11-advanced",
     "STORY VALUES",
     "Every global the game's scripts read, and a comparison with another save to fix a stuck quest."),
    ("12-kotor-2", "shot", "KOTOR II", "12-k2-saves",
     "SAVES  ·  KOTOR II",
     "KotOR II in its own colours, Steam Cloud saves included."),
    ("13-safe", "card", "YOUR SAVES ARE SAFE", None, None, None),
)

SAFE_LINES = (
    ("NOTHING IS WRITTEN UNTIL SAVE CHANGES", "Every change waits in memory. Revert reads the save from disk again."),
    ("A BACKUP FIRST, EVERY TIME", "The whole save folder is copied and checked before anything is written."),
    ("THE GAME HAS TO BE CLOSED", "Save Changes is refused while KotOR or KotOR II runs."),
    ("ONLY WELL-FORMED CHANGES", "Every field, its type and its order is kept as the game wrote it."),
)


def font(name, size):
    return ImageFont.truetype(os.path.join(ROOT, "fonts", name), size)


MENU = "KMRP-OriginalHD-Menu.otf"
LABEL = "KMRP-OriginalHD-MenuSmallB.otf"
BODY = "KMRP-OriginalHD-Descriptions.otf"


def layer():
    return Image.new("RGBA", (W, H), (0, 0, 0, 0))


def glow(img, blur, strength=1.0):
    g = img.filter(ImageFilter.GaussianBlur(blur))
    if strength != 1.0:
        g.putalpha(g.getchannel("A").point(lambda v: min(255, int(v * strength))))
    return g


def radial(center, radius, color, alpha):
    im = layer()
    d = ImageDraw.Draw(im)
    cx, cy = center
    for i in range(60, 0, -1):
        r = radius * i / 60
        d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=color + (int(alpha * (1 - i / 60) ** 1.6),))
    return im.filter(ImageFilter.GaussianBlur(8))


def tracked(d, x, y, text, f, fill, tracking, anchor="l"):
    """Letter-spaced text on a baseline; a space takes half its width."""
    widths = [d.textlength(c, font=f) * (0.5 if c == " " else 1) for c in text]
    total = sum(widths) + tracking * (len(text) - 1)
    if anchor == "c":
        x -= total / 2
    elif anchor == "r":
        x -= total
    for c, w in zip(text, widths):
        d.text((x, y), c, font=f, fill=fill, anchor="ls")
        x += w + tracking
    return total


def ground(k2=False):
    """Navy, the timeline's stars, a soft glow and darker edges."""
    base = Image.new("RGB", (W, H), WINDOW)
    field = Image.open(os.path.join(ROOT, "assets", "timeline", "stars.png")).convert("RGB")
    field = field.crop((0, 0, 1200, 675)).resize((W, H), Image.LANCZOS)
    base = ImageChops.screen(base, field.point(lambda v: int(v * 0.6))).convert("RGBA")
    base.alpha_composite(radial((W * 0.5, H * 0.55), 900, (20, 52, 44) if k2 else GLOW, 190))
    vig = Image.new("RGBA", (W, H), (3, 6, 12, 170))
    hole = Image.new("L", (W, H), 0)
    ImageDraw.Draw(hole).ellipse((-200, -260, W + 200, H + 260), fill=255)
    vig.putalpha(ImageChops.subtract(vig.getchannel("A"), hole.filter(ImageFilter.GaussianBlur(160))))
    base.alpha_composite(vig)
    return base


def brackets(im, box, line, accent, length=34, width=3, dot=True, halo=True):
    """The game's menu frame corners: bright L-brackets with a small square inside."""
    x0, y0, x1, y1 = box
    br = layer()
    d = ImageDraw.Draw(br)
    for cx, cy, sx, sy in ((x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)):
        d.line((cx, cy, cx + sx * length, cy), fill=line + (255,), width=width)
        d.line((cx, cy, cx, cy + sy * length), fill=line + (255,), width=width)
        if dot:
            d.rectangle((cx + sx * 8 - 2, cy + sy * 8 - 2, cx + sx * 8 + 2, cy + sy * 8 + 2), fill=accent + (255,))
    if halo:
        im.alpha_composite(glow(br, 4, 1.3))
    im.alpha_composite(br)


def page_frame(im, k2=False):
    d = ImageDraw.Draw(im)
    m = 22
    box = (m, m, W - m - 1, H - m - 1)
    d.rectangle(box, outline=(K2_LINE if k2 else FRAME_DIM) + (200,), width=1)
    brackets(im, box, K2_LINE if k2 else FRAME_LINE, K2_ACCENT if k2 else ACCENT, length=44)


def silver(im, x, baseline, cap, text="KMSE", anchor="l"):
    """Text in the crest's silver: Georgia, KMRP's metal ramp, a soft shadow."""
    f = ImageFont.truetype(GEORGIA, round(cap / 0.69))
    tracking = 0.12 * cap
    mask = Image.new("L", (W, H), 0)
    md = ImageDraw.Draw(mask)
    widths = [md.textlength(c, font=f) for c in text]
    total = sum(widths) + tracking * (len(text) - 1)
    if anchor == "c":
        x -= total / 2
    xx = x
    for c, w in zip(text, widths):
        md.text((xx, baseline), c, font=f, fill=255, anchor="ls")
        xx += w + tracking
    top = baseline - cap
    stops = [(0.0, (238, 242, 247)), (0.45, (150, 162, 178)), (0.72, (206, 196, 180)), (1.0, (118, 128, 144))]
    face = layer()
    fd = ImageDraw.Draw(face)
    for y in range(int(top) - 4, int(baseline) + 8):
        t = min(1.0, max(0.0, (y - top) / cap))
        for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
            if t0 <= t <= t1:
                k = (t - t0) / (t1 - t0)
                fd.line([(0, y), (W, y)], fill=tuple(round(a + (b - a) * k) for a, b in zip(c0, c1)) + (255,))
                break
    shadow = Image.new("RGBA", (W, H), (3, 6, 12, 0))
    shadow.putalpha(mask.filter(ImageFilter.GaussianBlur(cap * 0.12)).point(lambda v: int(v * 0.85)))
    im.alpha_composite(shadow, (0, max(2, round(cap * 0.05))))
    im.paste(face, (0, 0), mask)
    return x + total


def cyan_text(im, x, baseline, text, f, color=ACCENT, tracking=0, anchor="l", halo=8):
    t = layer()
    w = tracked(ImageDraw.Draw(t), x, baseline, text, f, color + (255,), tracking, anchor)
    if halo:
        im.alpha_composite(glow(t, halo, 1.4))
    im.alpha_composite(t)
    return w


def rule(im, x0, x1, y, color, bright=None):
    d = ImageDraw.Draw(im)
    d.line((x0, y, x1, y), fill=color + (200,), width=1)
    if bright:
        b0, b1 = bright
        r = layer()
        rd = ImageDraw.Draw(r)
        n = 200
        for i in range(n):
            t = i / (n - 1)
            a = int(255 * (1 - abs(t - 0.5) * 2) ** 1.2)
            xa = b0 + (b1 - b0) * i / n
            rd.line((xa, y, xa + (b1 - b0) / n + 0.5, y), fill=ACCENT + (a,), width=2)
        im.alpha_composite(glow(r, 3, 1.2))
        im.alpha_composite(r)


def header(im, title, k2=False):
    """KMSE in silver and the version in holo cyan at the left, the page's title at the right."""
    accent = K2_ACCENT if k2 else ACCENT
    end = silver(im, 72, 112, 46)
    cyan_text(im, end + 22, 112, VERSION, font(MENU, 50), accent, tracking=4)
    cyan_text(im, W - 72, 104, title, font(MENU, 34), accent, tracking=7, anchor="r", halo=6)
    rule(im, 72, W - 72, 146, K2_LINE if k2 else FRAME_DIM, bright=(72, 420))


def holo_frame(im, shot, box, k2=False):
    """A screenshot, scaled down whole, in a lit frame with corner brackets."""
    line = K2_LINE if k2 else FRAME_LINE
    accent = K2_ACCENT if k2 else ACCENT
    x0, y0, x1, y1 = box
    k = min((x1 - x0) / shot.width, (y1 - y0) / shot.height)
    sw, sh = round(shot.width * k), round(shot.height * k)
    sx, sy = round((x0 + x1 - sw) / 2), y0
    # a lit plate behind the window
    plate = layer()
    ImageDraw.Draw(plate).rectangle((sx - 6, sy - 6, sx + sw + 6, sy + sh + 6), fill=accent + (90,))
    im.alpha_composite(glow(plate, 26, 1.0))
    im.alpha_composite(shot.resize((sw, sh), Image.LANCZOS), (sx, sy))
    d = ImageDraw.Draw(im)
    d.rectangle((sx - 1, sy - 1, sx + sw, sy + sh), outline=line + (255,), width=2)
    brackets(im, (sx - 12, sy - 12, sx + sw + 11, sy + sh + 11), line, accent, length=30, width=3, dot=False)
    return sx, sy, sw, sh


def shot_page(shots, page):
    name, kind, title, shot, label, caption = page
    k2 = shot.startswith("12-")
    im = ground(k2)
    page_frame(im, k2)
    header(im, title, k2)
    window = Image.open(os.path.join(shots, shot + ".png")).convert("RGBA")
    sx, sy, sw, sh = holo_frame(im, window, (150, 196, W - 150, 900), k2)
    d = ImageDraw.Draw(im)
    lab = font(LABEL, 19)
    tracked(d, sx, sy + sh + 46, label, lab, (K2_ACCENT if k2 else ACCENT) + (255,), 5)
    tracked(d, sx + sw, sy + sh + 46, f"MACOS  ·  {window.width // 2}x{window.height // 2}", lab, SOFT + (255,), 5, "r")
    # one line, the type made smaller until it fits
    size = 34
    while size > 24 and d.textlength(caption, font=font(BODY, size)) > W - 260:
        size -= 1
    d.text((W / 2, 1010), caption, font=font(BODY, size), fill=TEXT + (255,), anchor="ms")
    return im


def cover():
    im = ground()
    page_frame(im)
    # the holocron, lit from below
    art = Image.open(os.path.join(ROOT, "assets", "icon", "art.png")).convert("RGBA")
    art = art.crop(art.getbbox())
    k = 400 / art.height
    art = art.resize((round(art.width * k), 400), Image.LANCZOS)
    ax, ay = (W - art.width) // 2, 84
    im.alpha_composite(radial((W / 2, ay + 230), 380, ACCENT, 90))
    im.alpha_composite(art, (ax, ay))
    # KMSE 1.0
    cap = 140
    f = ImageFont.truetype(GEORGIA, round(cap / 0.69))
    probe = ImageDraw.Draw(layer())
    word = sum(probe.textlength(c, font=f) for c in "KMSE") + 0.12 * cap * 3
    ver = probe.textlength(VERSION, font=font(MENU, 120)) + 3 * 6
    x = (W - (word + 40 + ver)) / 2
    end = silver(im, x, 660, cap)
    cyan_text(im, end + 40, 660, VERSION, font(MENU, 120), tracking=6, halo=14)
    d = ImageDraw.Draw(im)
    tracked(d, W / 2, 728, "KOTOR MODERN GAMESAVE EDITOR", font(LABEL, 30), MUTED + (255,), 12, "c")
    rule(im, W / 2 - 420, W / 2 + 420, 760, FRAME_DIM, bright=(W / 2 - 300, W / 2 + 300))
    lab = font(LABEL, 24)
    tracked(d, W / 2, 830, "CHARACTER  ·  ABILITIES  ·  INVENTORY  ·  PARTY  ·  TIMELINE", lab, TEXT + (255,), 7, "c")
    tracked(d, W / 2, 878, "QUESTS  ·  PAZAAK & DESTINATIONS  ·  MAP  ·  STORY VALUES", lab, TEXT + (255,), 7, "c")
    t = layer()
    td = ImageDraw.Draw(t)
    tracked(td, W / 2, 950, "KOTOR  ·  KOTOR II", font(LABEL, 26), ACCENT + (255,), 9, "c")
    im.alpha_composite(glow(t, 6, 1.2))
    im.alpha_composite(t)
    tracked(d, W / 2, 1000, "WINDOWS  ·  MACOS  ·  LINUX", font(LABEL, 22), SOFT + (255,), 9, "c")
    return im


def safe_card():
    im = ground()
    page_frame(im)
    header(im, "YOUR SAVES ARE SAFE")
    d = ImageDraw.Draw(im)
    top = 250
    for i, (head, line) in enumerate(SAFE_LINES):
        y = top + i * 170
        box = (300, y, W - 300, y + 130)
        p = layer()
        ImageDraw.Draw(p).rectangle(box, fill=(9, 17, 30, 220))
        im.alpha_composite(p)
        d.rectangle(box, outline=FRAME_DIM + (255,), width=1)
        brackets(im, box, FRAME_LINE, ACCENT, length=22, width=2, dot=False, halo=False)
        cyan_text(im, 360, y + 56, head, font(MENU, 30), tracking=4, halo=5)
        d.text((360, y + 100), line, font=font(BODY, 30), fill=MUTED + (255,), anchor="ls")
    body = font(BODY, 30)
    d.text((W / 2, 1000), "Original Xbox saves can be looked at, not changed: the Xbox checks a signature on every save.",
           font=body, fill=SOFT + (255,), anchor="ms")
    return im


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("shots")
    ap.add_argument("--only", default="")
    ap.add_argument("--out", default=OUT)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    for page in PAGES:
        if not page[0].startswith(a.only):
            continue
        im = cover() if page[1] == "cover" else safe_card() if page[1] == "card" else shot_page(a.shots, page)
        path = os.path.join(a.out, page[0] + ".png")
        im.convert("RGB").save(path, optimize=True)
        print("wrote", path)


if __name__ == "__main__":
    main()
