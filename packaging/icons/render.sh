#!/bin/sh
# Renders the icon (moonglow-viewer.svg) into what the packages need: PNGs
# for the window and the Linux icon theme, moonglow-viewer.ico (Windows) and
# moonglow-viewer.icns (macOS). Needs rsvg-convert and Pillow (python3).
# The outputs are committed, so builds never run this.
set -eu
cd "$(dirname "$0")"
for s in 16 24 32 48 64 128 256 512; do
    rsvg-convert -w $s -h $s moonglow-viewer.svg -o moonglow-viewer-$s.png
done
python3 - <<'PY'
from PIL import Image
big = Image.open("moonglow-viewer-512.png")
sizes = [16, 24, 32, 48, 64, 128, 256]
big.save("moonglow-viewer.ico", sizes=[(s, s) for s in sizes],
         append_images=[Image.open(f"moonglow-viewer-{s}.png") for s in sizes])
big.save("moonglow-viewer.icns")
PY
