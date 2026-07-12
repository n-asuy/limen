#!/usr/bin/env python3
"""
Generate tray digit icons (1-9 and dot) as 32x32 RGBA PNGs with transparent background
and solid black glyphs. Optionally draw a circular ring around the glyph.
Outputs to apps/desktop/src-tauri/icons/tray_digits/ by default.

No external dependencies required; uses a minimal PNG writer.

Usage:
  python3 apps/desktop/src-tauri/scripts/gen_tray_digits.py
  python3 apps/desktop/src-tauri/scripts/gen_tray_digits.py --out apps/desktop/src-tauri/icons/tray_digits --size 32 --pad 4 --thick 3 --circle --ring 2
"""
import argparse
import os
import struct
import zlib


def write_png_rgba(path: str, width: int, height: int, rgba: bytes) -> None:
    assert len(rgba) == width * height * 4

    def chunk(typ: bytes, data: bytes) -> bytes:
        return (
            struct.pack('>I', len(data))
            + typ
            + data
            + struct.pack('>I', zlib.crc32(typ + data) & 0xFFFFFFFF)
        )

    # PNG signature
    sig = b"\x89PNG\r\n\x1a\n"
    # IHDR
    ihdr = struct.pack(
        '>IIBBBBB',
        width,
        height,
        8,   # bit depth
        6,   # color type RGBA
        0,   # compression
        0,   # filter
        0,   # interlace
    )

    # IDAT: add filter byte 0 at each scanline
    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)  # filter type 0 (None)
        row = rgba[y * stride:(y + 1) * stride]
        raw.extend(row)
    comp = zlib.compress(bytes(raw), level=9)

    png = bytearray()
    png += sig
    png += chunk(b'IHDR', ihdr)
    png += chunk(b'IDAT', comp)
    png += chunk(b'IEND', b'')

    with open(path, 'wb') as f:
        f.write(png)


def gen_canvas(w: int, h: int) -> bytearray:
    # Start with fully transparent
    return bytearray([0, 0, 0, 0] * (w * h))


def fill_rect(rgba: bytearray, w: int, h: int, x0: int, y0: int, x1: int, y1: int):
    xa = max(0, min(x0, x1))
    xb = min(w, max(x0, x1))
    ya = max(0, min(y0, y1))
    yb = min(h, max(y0, y1))
    for y in range(ya, yb):
        off = (y * w + xa) * 4
        for x in range(xa, xb):
            rgba[off + 0] = 0   # R
            rgba[off + 1] = 0   # G
            rgba[off + 2] = 0   # B
            rgba[off + 3] = 255 # A
            off += 4


def draw_seven_segment_digit(w: int, h: int, pad: int, thick: int, digit: int, *, cap: int = 0, inset: int = 0) -> bytes:
    # 24x24 content box inside padding by default when size=32 and pad=4
    seg_w = w - pad * 2
    seg_h = h - pad * 2
    top_y = pad
    mid_y = pad + seg_h // 2 - thick // 2
    bot_y = pad + seg_h - thick
    left_x = pad + inset
    right_x = pad + seg_w - thick - inset

    rgba = gen_canvas(w, h)

    def seg(ch: str):
        if ch == 'a':   # top horizontal (shrink ends by cap)
            fill_rect(rgba, w, h, left_x + cap, top_y + cap, left_x + seg_w - cap, top_y + cap + thick)
        elif ch == 'd': # bottom horizontal
            fill_rect(rgba, w, h, left_x + cap, bot_y - cap, left_x + seg_w - cap, bot_y - cap + thick)
        elif ch == 'g': # middle horizontal
            fill_rect(rgba, w, h, left_x + cap, mid_y, left_x + seg_w - cap, mid_y + thick)
        elif ch == 'f': # top-left vertical
            fill_rect(rgba, w, h, left_x, top_y + cap, left_x + thick, top_y + seg_h // 2 - cap)
        elif ch == 'b': # top-right vertical
            fill_rect(rgba, w, h, right_x, top_y + cap, right_x + thick, top_y + seg_h // 2 - cap)
        elif ch == 'e': # bottom-left vertical
            fill_rect(rgba, w, h, left_x, top_y + seg_h // 2 + cap, left_x + thick, top_y + seg_h - cap)
        elif ch == 'c': # bottom-right vertical
            fill_rect(rgba, w, h, right_x, top_y + seg_h // 2 + cap, right_x + thick, top_y + seg_h - cap)

    mapping = {
        0: 'abdefc',
        1: 'bc',
        2: 'abged',
        3: 'abgcd',
        4: 'fgbc',
        5: 'afgcd',
        6: 'afgecd',
        7: 'abc',
        8: 'abdefgc',
        9: 'abfgcd',
    }
    for ch in mapping.get(digit, ''):
        seg(ch)
    return bytes(rgba)


def draw_dot(w: int, h: int, size: int = 4) -> bytes:
    rgba = gen_canvas(w, h)
    cx, cy = w // 2, h // 2
    x0 = cx - size // 2
    y0 = cy - size // 2
    fill_rect(rgba, w, h, x0, y0, x0 + size, y0 + size)
    return bytes(rgba)


def draw_ring(rgba: bytearray, w: int, h: int, cx: int, cy: int, radius: float, ring: int):
    if ring <= 0:
        return
    r_out = radius
    r_in = max(0.0, radius - ring)
    r_out2 = r_out * r_out
    r_in2 = r_in * r_in
    for y in range(h):
        dy = (y + 0.5) - cy
        off = (y * w) * 4
        for x in range(w):
            dx = (x + 0.5) - cx
            d2 = dx * dx + dy * dy
            if r_in2 <= d2 <= r_out2:
                rgba[off + 0] = 0
                rgba[off + 1] = 0
                rgba[off + 2] = 0
                rgba[off + 3] = 255
            off += 4


def main():
    parser = argparse.ArgumentParser()
    default_out = os.path.normpath(
        os.path.join(os.path.dirname(__file__), '..', 'icons', 'tray_digits')
    )
    parser.add_argument('--out', default=default_out, help='output directory for PNGs')
    parser.add_argument('--size', type=int, default=32, help='final canvas size (square)')
    parser.add_argument('--supersample', type=int, default=1, help='supersampling factor (e.g. 4 for smoother edges)')
    parser.add_argument('--pad', type=int, default=4, help='padding around glyph')
    parser.add_argument('--thick', type=int, default=3, help='segment thickness')
    parser.add_argument('--dot', type=int, default=4, help='dot size (px)')
    parser.add_argument('--circle', action='store_true', help='draw a circular ring around glyph')
    parser.add_argument('--ring', type=int, default=2, help='ring thickness (px)')
    parser.add_argument('--cap', type=int, default=2, help='gap (px) between glyph segments and circle')
    parser.add_argument('--inset', type=int, default=None, help='shrink glyph horizontally (px) to fit circle; default auto when --circle')
    args = parser.parse_args()

    # supersampled working canvas
    ss = max(1, int(args.supersample))
    w = h = args.size * ss
    os.makedirs(args.out, exist_ok=True)

    # digits 1..9
    def downsample_rgba(rgba: bytes, w: int, h: int, ss: int) -> bytes:
        if ss == 1:
            return rgba
        W = w // ss
        H = h // ss
        out = bytearray(W * H * 4)
        stride = w * 4
        for oy in range(H):
            for ox in range(W):
                r = g = b = a = 0
                for yy in range(ss):
                    row = (oy * ss + yy) * stride
                    off = row + (ox * ss) * 4
                    for xx in range(ss):
                        r += rgba[off + 0]
                        g += rgba[off + 1]
                        b += rgba[off + 2]
                        a += rgba[off + 3]
                        off += 4
                n = ss * ss
                i = (oy * W + ox) * 4
                out[i + 0] = r // n
                out[i + 1] = g // n
                out[i + 2] = b // n
                out[i + 3] = a // n
        return bytes(out)

    def save_png(path: str, rgba: bytes):
        png_rgba = downsample_rgba(rgba, w, h, ss)
        write_png_rgba(path, args.size, args.size, png_rgba)

    for d in range(1, 10):
        # ensure content area does not touch the ring
        pad = (max(args.pad, args.ring + args.cap + 2) if args.circle else args.pad) * ss
        inset = (args.inset if args.inset is not None else (1 if args.circle else 0)) * ss
        thick = args.thick * ss
        cap = (args.cap if args.circle else 0) * ss
        rgba = bytearray(draw_seven_segment_digit(w, h, pad, thick, d, cap=cap, inset=inset))
        if args.circle:
            draw_ring(rgba, w, h, w // 2, h // 2, min(w, h) / 2 - 1.5 * ss, args.ring * ss)
        path = os.path.join(args.out, f"{d}.png")
        save_png(path, bytes(rgba))
        print(f"wrote {path}")

    # dot
    rgba = bytearray(draw_dot(w, h, args.dot * ss))
    if args.circle:
        draw_ring(rgba, w, h, w // 2, h // 2, min(w, h) / 2 - 1.5 * ss, args.ring * ss)
    path = os.path.join(args.out, "dot.png")
    save_png(path, bytes(rgba))
    print(f"wrote {path}")


if __name__ == '__main__':
    main()
