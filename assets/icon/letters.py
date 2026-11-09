"""KMSE in the icon's letters, made by the logo's own script: KMRP's tools/build_brand_image.py, which made the
KMSE crest (`../kmse-logo.png`) and KMRP's header the same way. It is run unchanged, with KMSE, the letters alone
(no crest) and the KMRP wordmark's tracking, 0.12 (measured on the crest's letters: 31 px tall, 134 px wide; the
script gives 135 there). Only the cap height is the icon's: 216 px, the word about 970 px wide.

    python letters.py "E:\\KMRP - KOTOR Modern Restoration Patch"

writes kmse-letters.png here (Georgia, from C:\\Windows\\Fonts, as the script expects).
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
kmrp = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(r"E:\KMRP - KOTOR Modern Restoration Patch")
sys.path.insert(0, str(kmrp / "tools"))
import build_brand_image as brand  # noqa: E402

brand.CAP = 216
brand.TRACKING = 0.12
brand.build(None, HERE / "kmse-letters.png", brand.DEFAULT_FONT, "KMSE")
