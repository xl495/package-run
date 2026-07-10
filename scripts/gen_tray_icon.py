"""Generate a monochrome ">_" template tray icon (44x44 PNG) using only the stdlib."""
import struct, zlib, math, sys

SIZE = 44
STROKE = 2.4  # stroke radius in px

# terminal prompt glyph: chevron ">" + underscore "_"
SEGMENTS = [
    ((10, 12), (21, 22)),
    ((21, 22), (10, 32)),
    ((25, 32), (35, 32)),
]

def dist_to_segment(px, py, a, b):
    ax, ay = a; bx, by = b
    dx, dy = bx - ax, by - ay
    l2 = dx * dx + dy * dy
    if l2 == 0:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / l2))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))

rows = []
for y in range(SIZE):
    row = bytearray()
    for x in range(SIZE):
        d = min(dist_to_segment(x + 0.5, y + 0.5, a, b) for a, b in SEGMENTS)
        alpha = max(0.0, min(1.0, STROKE + 0.6 - d))
        row += bytes((0, 0, 0, int(alpha * 255)))
    rows.append(bytes(row))

raw = b"".join(b"\x00" + r for r in rows)

def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9))
       + chunk(b"IEND", b""))

with open(sys.argv[1], "wb") as f:
    f.write(png)
print("wrote", sys.argv[1])
