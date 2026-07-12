#!/usr/bin/env python3
"""Generate the menu bar tray icons: nine Spaces, the current one lit.

The dots sit in a keypad grid, so their positions match the 1-9 keys that jump
between Spaces. Showing all nine says more than a numeral can: where you are,
and how much is around you. The inactive dots stay dim rather than absent, so
the mark keeps its shape whichever Space is active. `dot.png` lights none of
them and stands in whenever the active Space is unknown.

Output is a macOS template image: pure black pixels with an alpha mask, which
the system re-tints for light, dark and highlighted menu bars. Dim dots are
carried by that alpha channel.

    python3 tooling/scripts/generate_tray_icons.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 32  # menu bar icons are square; macOS scales them to bar height
SUPERSAMPLE = 8
N = SIZE * SUPERSAMPLE

OUT_DIR = Path(__file__).resolve().parents[2] / "src-tauri" / "icons" / "tray_digits"

# Geometry as fractions of the canvas.
DOT_RADIUS = 0.088
DOT_STEP = 0.30  # centre to centre
ACTIVE_SCALE = 1.35
INACTIVE_ALPHA = 90  # against 255 for the active dot


def render(active: int | None) -> Image.Image:
    coverage = Image.new("L", (N, N), 0)
    draw = ImageDraw.Draw(coverage)

    radius = N * DOT_RADIUS
    step = N * DOT_STEP
    origin = N / 2 - step  # top-left dot centre

    for index in range(9):
        row, col = divmod(index, 3)
        cx = origin + col * step
        cy = origin + row * step

        lit = index + 1 == active
        r = radius * (ACTIVE_SCALE if lit else 1.0)
        alpha = 255 if lit else INACTIVE_ALPHA
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=alpha)

    mask = coverage.resize((SIZE, SIZE), Image.LANCZOS)
    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    icon.paste((0, 0, 0), (0, 0), mask)
    icon.putalpha(mask)
    return icon


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for space in range(1, 10):
        render(space).save(OUT_DIR / f"{space}.png")
    render(None).save(OUT_DIR / "dot.png")
    print(f"wrote 10 tray icons to {OUT_DIR}")


if __name__ == "__main__":
    main()
