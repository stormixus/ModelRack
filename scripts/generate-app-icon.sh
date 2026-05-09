#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SVG="$ROOT/assets/AppIcon.svg"
ICONSET="$ROOT/assets/AppIcon.iconset"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

qlmanage -r cache >/dev/null 2>&1 || true
cp "$SVG" "$WORK/AppIcon.svg"
qlmanage -t -s 1024 -o "$WORK" "$WORK/AppIcon.svg" >/dev/null
SRC="$WORK/AppIcon.svg.png"
MASKED="$WORK/AppIcon.masked.png"

python3 - "$SRC" "$MASKED" <<'PY'
import struct
import sys
import zlib

src, dst = sys.argv[1], sys.argv[2]


def paeth(a, b, c):
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def read_png(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path}: not a PNG")

    pos = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    while pos < len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        pos += 4
        chunk_type = data[pos:pos + 4]
        pos += 4
        chunk = data[pos:pos + length]
        pos += length + 4
        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type, _compression, _filter, interlace = struct.unpack(">IIBBBBB", chunk)
        elif chunk_type == b"IDAT":
            idat.extend(chunk)
        elif chunk_type == b"IEND":
            break

    if bit_depth != 8 or color_type != 6 or interlace != 0:
        raise SystemExit(f"{path}: expected non-interlaced 8-bit RGBA PNG")

    bpp = 4
    stride = width * bpp
    decoded = zlib.decompress(bytes(idat))
    rows = []
    prev = [0] * stride
    cursor = 0
    for _y in range(height):
        filter_type = decoded[cursor]
        cursor += 1
        scanline = list(decoded[cursor:cursor + stride])
        cursor += stride
        out = [0] * stride
        for i, value in enumerate(scanline):
            left = out[i - bpp] if i >= bpp else 0
            up = prev[i]
            upper_left = prev[i - bpp] if i >= bpp else 0
            if filter_type == 0:
                out[i] = value
            elif filter_type == 1:
                out[i] = (value + left) & 0xff
            elif filter_type == 2:
                out[i] = (value + up) & 0xff
            elif filter_type == 3:
                out[i] = (value + ((left + up) // 2)) & 0xff
            elif filter_type == 4:
                out[i] = (value + paeth(left, up, upper_left)) & 0xff
            else:
                raise SystemExit(f"Unsupported PNG filter: {filter_type}")
        rows.append(out)
        prev = out
    return width, height, rows


def write_png(path, width, height, rows):
    def chunk(chunk_type, payload):
        return (
            struct.pack(">I", len(payload))
            + chunk_type
            + payload
            + struct.pack(">I", zlib.crc32(chunk_type + payload) & 0xffffffff)
        )

    raw = bytearray()
    for row in rows:
        raw.append(0)
        raw.extend(row)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
        f.write(chunk(b"IEND", b""))


width, height, rows = read_png(src)
if width != 1024 or height != 1024:
    raise SystemExit(f"{src}: expected 1024x1024 source, got {width}x{height}")

# qlmanage rasterizes transparent SVG corners against white. Re-apply the app
# icon shape as real alpha so the Dock does not show a white halo while running.
left = top = 72.0
right = bottom = 952.0
radius = 176.0
samples = (0.25, 0.75)
edge_rgb = (8, 22, 31)


def inside_round_rect(px, py):
    cx = min(max(px, left + radius), right - radius)
    cy = min(max(py, top + radius), bottom - radius)
    return (px - cx) ** 2 + (py - cy) ** 2 <= radius ** 2


for y, row in enumerate(rows):
    for x in range(width):
        covered = 0
        for sy in samples:
            for sx in samples:
                if inside_round_rect(x + sx, y + sy):
                    covered += 1
        alpha = round(255 * covered / (len(samples) ** 2))
        i = x * 4
        if alpha == 0:
            row[i:i + 4] = [0, 0, 0, 0]
        elif alpha < 255:
            r, g, b = row[i], row[i + 1], row[i + 2]
            if (r + g + b) / 3 > 180:
                row[i], row[i + 1], row[i + 2] = edge_rgb
            row[i + 3] = alpha
        else:
            row[i + 3] = 255

write_png(dst, width, height, rows)
PY
SRC="$MASKED"

mkdir -p "$ICONSET"
sips -z 16 16 "$SRC" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$SRC" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$SRC" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$SRC" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$SRC" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$SRC" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$SRC" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$SRC" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$SRC" --out "$ICONSET/icon_512x512.png" >/dev/null
cp "$SRC" "$ICONSET/icon_512x512@2x.png"
iconutil -c icns "$ICONSET" -o "$ROOT/assets/AppIcon.icns"
printf 'Generated %s and %s\n' "$ICONSET" "$ROOT/assets/AppIcon.icns"
