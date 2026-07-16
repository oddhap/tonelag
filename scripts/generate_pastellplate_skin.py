"""Generate Tonelag's original Pastellplate classic skin reproducibly."""

from pathlib import Path
import struct
import zlib
import zipfile


WIDTH = 275
HEIGHT = 116
CREAM = (255, 248, 231)
CREAM_DARK = (245, 229, 205)
LAVENDER = (188, 162, 245)
LAVENDER_DARK = (153, 124, 220)
BLUE = (146, 200, 255)
PINK = (248, 180, 210)
PEACH = (255, 208, 157)
PLUM = (62, 49, 90)
PLUM_SOFT = (86, 70, 111)
WHITE = (255, 253, 247)


class Canvas:
    def __init__(self, width: int, height: int, fill=CREAM):
        self.width = width
        self.height = height
        self.pixels = [[fill for _ in range(width)] for _ in range(height)]

    def set(self, x: int, y: int, color) -> None:
        if 0 <= x < self.width and 0 <= y < self.height:
            self.pixels[y][x] = color

    def rect(self, x: int, y: int, width: int, height: int, color) -> None:
        for yy in range(max(0, y), min(self.height, y + height)):
            for xx in range(max(0, x), min(self.width, x + width)):
                self.pixels[yy][xx] = color

    def frame(self, x: int, y: int, width: int, height: int, color, inner=WHITE) -> None:
        self.rect(x, y, width, height, color)
        self.rect(x + 1, y + 1, width - 2, height - 2, inner)

    def line(self, x0: int, y0: int, x1: int, y1: int, color) -> None:
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        error = dx + dy
        while True:
            self.set(x0, y0, color)
            if x0 == x1 and y0 == y1:
                break
            twice = 2 * error
            if twice >= dy:
                error += dy
                x0 += sx
            if twice <= dx:
                error += dx
                y0 += sy

    def circle(self, cx: int, cy: int, radius: int, color) -> None:
        for y in range(cy - radius, cy + radius + 1):
            for x in range(cx - radius, cx + radius + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2:
                    self.set(x, y, color)

    def bmp(self) -> bytes:
        padding = (-self.width * 3) % 4
        rows = bytearray()
        for row in reversed(self.pixels):
            for red, green, blue in row:
                rows.extend((blue, green, red))
            rows.extend(b"\0" * padding)
        offset = 54
        header = b"BM" + struct.pack("<IHHI", offset + len(rows), 0, 0, offset)
        dib = struct.pack(
            "<IiiHHIIiiII", 40, self.width, self.height, 1, 24, 0,
            len(rows), 2835, 2835, 0, 0,
        )
        return header + dib + rows

    def png(self) -> bytes:
        rows = []
        for row in self.pixels:
            scanline = bytearray([0])
            for red, green, blue in row:
                scanline.extend((red, green, blue, 255))
            rows.append(scanline)
        raw = b"".join(rows)
        header = struct.pack(">IIBBBBB", self.width, self.height, 8, 6, 0, 0, 0)
        return (
            b"\x89PNG\r\n\x1a\n"
            + png_chunk(b"IHDR", header)
            + png_chunk(b"IDAT", zlib.compress(raw, 9))
            + png_chunk(b"IEND", b"")
        )


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def shell(title_accent=LAVENDER) -> Canvas:
    canvas = Canvas(WIDTH, HEIGHT)
    canvas.rect(0, 0, WIDTH, HEIGHT, PLUM)
    canvas.rect(1, 1, WIDTH - 2, HEIGHT - 2, CREAM)
    canvas.rect(2, 2, WIDTH - 4, 12, title_accent)
    canvas.rect(3, 3, WIDTH - 6, 2, WHITE)
    for x in range(7, 110, 8):
        canvas.rect(x, 7, 5, 2, CREAM)
    for x in range(165, 258, 8):
        canvas.rect(x, 7, 5, 2, CREAM)
    canvas.circle(137, 8, 4, CREAM)
    canvas.circle(137, 8, 2, PINK)
    canvas.rect(4, 15, WIDTH - 8, 2, CREAM_DARK)
    return canvas


def main_bitmap() -> Canvas:
    canvas = shell(LAVENDER)
    canvas.frame(12, 21, 251, 50, PLUM, PLUM_SOFT)
    canvas.rect(15, 24, 245, 44, (46, 38, 67))
    canvas.rect(18, 27, 68, 36, (52, 43, 76))
    canvas.rect(90, 27, 167, 16, (58, 47, 82))
    for x, color in enumerate((PINK, PEACH, LAVENDER, BLUE) * 9):
        height = 5 + ((x * 7) % 15)
        canvas.rect(93 + x * 4, 63 - height, 2, height, color)
    canvas.rect(5, 76, 265, 35, CREAM_DARK)
    canvas.rect(8, 79, 139, 27, BLUE)
    canvas.rect(10, 81, 135, 23, CREAM)
    canvas.rect(153, 79, 114, 27, PINK)
    canvas.rect(155, 81, 110, 23, CREAM)
    for x, color in ((16, LAVENDER), (40, BLUE), (64, PINK), (88, PEACH), (112, LAVENDER)):
        canvas.circle(x, 93, 7, color)
        canvas.circle(x, 93, 3, WHITE)
    canvas.rect(163, 88, 91, 3, PLUM_SOFT)
    canvas.rect(163, 97, 91, 3, PLUM_SOFT)
    canvas.circle(224, 89, 5, BLUE)
    canvas.circle(191, 98, 5, PINK)
    return canvas


def eq_bitmap() -> Canvas:
    base = shell(BLUE)
    canvas = Canvas(WIDTH, 315, PLUM)
    canvas.pixels[:HEIGHT] = [list(row) for row in base.pixels]
    canvas.rect(7, 20, 261, 89, CREAM_DARK)
    canvas.rect(10, 23, 255, 83, CREAM)
    canvas.frame(14, 27, 35, 72, LAVENDER_DARK, (238, 226, 255))
    for x in range(60, 253, 19):
        canvas.rect(x, 30, 5, 64, PLUM_SOFT)
        canvas.rect(x + 1, 31, 3, 62, (224, 211, 239))
        knob_y = 43 + ((x * 3) % 35)
        canvas.rect(x - 3, knob_y, 11, 6, LAVENDER if (x // 19) % 2 else PINK)
        canvas.rect(x - 2, knob_y + 1, 9, 1, WHITE)
    for y in (34, 62, 90):
        canvas.rect(18, y, 27, 2, PLUM_SOFT)
    canvas.rect(25, 53, 12, 8, PEACH)
    # Standard EQMAIN sprite area used by Winamp 2.x compatible clients.
    for y, color in ((116, BLUE), (125, LAVENDER)):
        canvas.rect(0, y, 9, 9, color)
        canvas.line(2, y + 2, 6, y + 6, PLUM)
        canvas.line(6, y + 2, 2, y + 6, PLUM)
    for x, color in ((10, BLUE), (69, LAVENDER), (128, PEACH), (187, PINK)):
        canvas.rect(x, 119, 26, 12, color)
        canvas.rect(x + 2, 121, 22, 2, WHITE)
    canvas.rect(0, 164, 11, 11, LAVENDER)
    canvas.rect(2, 166, 7, 2, WHITE)
    canvas.rect(0, 176, 11, 11, LAVENDER_DARK)
    canvas.rect(224, 164, 44, 12, PINK)
    canvas.rect(226, 166, 40, 2, WHITE)
    canvas.rect(224, 176, 44, 12, PEACH)
    canvas.rect(0, 294, 113, 19, (46, 38, 67))
    for x in range(4, 110, 10):
        canvas.rect(x, 302 - (x % 4), 2, 6 + (x % 5), BLUE if x % 20 else PINK)
    canvas.rect(115, 294, 1, 19, LAVENDER)
    canvas.rect(0, 314, 113, 1, PEACH)
    return canvas


def playlist_bitmap() -> Canvas:
    base = shell(PINK)
    canvas = Canvas(280, 186, PLUM)
    for y, row in enumerate(base.pixels):
        canvas.pixels[y][:WIDTH] = list(row)
    canvas.frame(7, 20, 261, 88, PLUM_SOFT, WHITE)
    canvas.rect(10, 23, 255, 62, (255, 250, 238))
    for y in range(25, 84, 10):
        canvas.rect(12, y, 251, 1, (232, 218, 240))
    canvas.rect(10, 88, 255, 17, CREAM_DARK)
    for x, color in ((15, LAVENDER), (66, BLUE), (117, PINK), (168, PEACH), (219, LAVENDER)):
        canvas.rect(x, 92, 42, 9, color)
        canvas.rect(x + 1, 93, 40, 2, WHITE)
    # Menu sprites at the canonical PLEDIT coordinates.
    for x, color in ((0, BLUE), (23, LAVENDER), (54, PINK), (77, PEACH), (104, LAVENDER),
                     (127, BLUE), (154, PEACH), (177, PINK), (204, LAVENDER), (227, BLUE)):
        for y in (111, 130, 149):
            canvas.rect(x, y, 22, 18, PLUM)
            canvas.rect(x + 1, y + 1, 20, 16, color)
            canvas.rect(x + 3, y + 4, 16, 2, WHITE)
    return canvas


def titlebar_bitmap() -> Canvas:
    canvas = Canvas(320, 87, PLUM)
    for y, accent in ((0, LAVENDER), (15, BLUE)):
        canvas.rect(27, y, 275, 14, accent)
        canvas.rect(29, y + 2, 271, 2, WHITE)
        for x in range(37, 285, 8):
            canvas.rect(x, y + 7, 5, 2, CREAM)
    for x, color in ((0, BLUE), (9, LAVENDER), (18, PINK)):
        canvas.rect(x, 0, 9, 9, color)
        canvas.rect(x, 9, 9, 9, tuple(max(0, channel - 25) for channel in color))
    canvas.line(20, 2, 24, 6, PLUM)
    canvas.line(24, 2, 20, 6, PLUM)
    return canvas


def shufrep_bitmap() -> Canvas:
    canvas = Canvas(92, 85, PLUM)
    for y, shade in ((0, 0), (15, 20), (30, -12), (45, 8)):
        repeat = tuple(max(0, min(255, channel + shade)) for channel in BLUE)
        shuffle = tuple(max(0, min(255, channel + shade)) for channel in PINK)
        canvas.rect(0, y, 28, 15, repeat)
        canvas.rect(28, y, 47, 15, shuffle)
        canvas.rect(2, y + 2, 24, 2, WHITE)
        canvas.rect(30, y + 2, 43, 2, WHITE)
    for x, color in ((0, LAVENDER), (23, PEACH), (46, BLUE), (69, PINK)):
        canvas.rect(x, 61, 23, 12, color)
        canvas.rect(x, 73, 23, 12, tuple(max(0, channel - 20) for channel in color))
    return canvas


def posbar_bitmap() -> Canvas:
    canvas = Canvas(307, 10, CREAM_DARK)
    canvas.rect(0, 3, 248, 4, PLUM_SOFT)
    canvas.rect(1, 4, 246, 1, WHITE)
    canvas.rect(248, 0, 29, 10, LAVENDER)
    canvas.rect(250, 2, 25, 2, WHITE)
    canvas.rect(278, 0, 29, 10, LAVENDER_DARK)
    return canvas


def controls_bitmap() -> Canvas:
    canvas = Canvas(136, 36, CREAM_DARK)
    widths = [23, 23, 23, 23, 22, 22]
    colors = [LAVENDER, BLUE, PINK, PEACH, LAVENDER, BLUE]
    start = 0
    for index, width in enumerate(widths):
        for row in range(2):
            y = row * 18
            color = colors[index] if row == 0 else tuple(max(0, channel - 22) for channel in colors[index])
            canvas.rect(start, y, width, 18, PLUM)
            canvas.rect(start + 1, y + 1, width - 2, 16, color)
            canvas.rect(start + 2, y + 2, width - 4, 2, WHITE)
            cx, cy = start + width // 2, y + 9
            if index == 0:
                canvas.line(cx + 3, cy - 4, cx - 3, cy, PLUM)
                canvas.line(cx - 3, cy, cx + 3, cy + 4, PLUM)
            elif index in (1, 3):
                direction = 1 if index == 1 else -1
                for step in range(5):
                    canvas.line(cx - direction * 3, cy - 4 + step, cx + direction * 3, cy, PLUM)
                    canvas.line(cx + direction * 3, cy, cx - direction * 3, cy + 4 - step, PLUM)
            elif index == 2:
                canvas.rect(cx - 3, cy - 4, 3, 8, PLUM)
                canvas.rect(cx + 2, cy - 4, 3, 8, PLUM)
            elif index == 4:
                canvas.rect(cx - 4, cy - 4, 8, 8, PLUM)
            else:
                canvas.line(cx - 4, cy + 4, cx, cy - 4, PLUM)
                canvas.line(cx, cy - 4, cx + 4, cy + 4, PLUM)
        start += width
    return canvas


def glyph_bitmap(color, width=64, height=16) -> Canvas:
    canvas = Canvas(width, height, PLUM)
    for x in range(3, width - 3, 8):
        canvas.rect(x, 3, 5, 10, color)
        canvas.rect(x + 1, 4, 3, 2, WHITE)
    return canvas


def make_skin(output: Path, preview: Path) -> None:
    main = main_bitmap()
    equalizer = eq_bitmap()
    playlist = playlist_bitmap()
    files = {
        "MAIN.BMP": main.bmp(),
        "EQMAIN.BMP": equalizer.bmp(),
        "PLEDIT.BMP": playlist.bmp(),
        "CBUTTONS.BMP": controls_bitmap().bmp(),
        "TITLEBAR.BMP": titlebar_bitmap().bmp(),
        "SHUFREP.BMP": shufrep_bitmap().bmp(),
        "POSBAR.BMP": posbar_bitmap().bmp(),
        "TEXT.BMP": glyph_bitmap(LAVENDER).bmp(),
        "NUMBERS.BMP": glyph_bitmap(PEACH).bmp(),
        "PLEDIT.TXT": (
            b"[Text]\r\nNormal=#56466F\r\nCurrent=#3E315A\r\n"
            b"NormalBG=#FFF8E7\r\nSelectedBG=#E6D8FA\r\n"
        ),
        "VISCOLOR.TXT": b"248,180,210\r\n255,208,157\r\n188,162,245\r\n146,200,255\r\n255,248,231\r\n",
        "REGION.TXT": (
            b"[Normal]\r\nNumPoints=4\r\nPointList=0,0,275,0,275,116,0,116\r\n"
            b"[Equalizer]\r\nNumPoints=4\r\nPointList=0,0,275,0,275,116,0,116\r\n"
        ),
        "SKIN.HINTS": b"Pastellplate - an original Tonelag skin\r\n",
        "GENEX.COLS": b"window=#FFF8E7\r\nwindowtext=#56466F\r\nbutton=#BCA2F5\r\n",
        "LICENSE.txt": (
            b"Pastellplate artwork Copyright 2026 Tonelag contributors.\r\n"
            b"SPDX-License-Identifier: MIT OR Apache-2.0\r\n"
        ),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, files[name])

    stacked = Canvas(WIDTH, HEIGHT * 3)
    for offset, source in enumerate((main, equalizer, playlist)):
        for y, row in enumerate(source.pixels[:HEIGHT]):
            stacked.pixels[offset * HEIGHT + y] = list(row[:WIDTH])
    preview.parent.mkdir(parents=True, exist_ok=True)
    preview.write_bytes(stacked.png())


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    make_skin(
        root / "assets" / "default-skin" / "pastellplate.wsz",
        root / "assets" / "branding" / "pastellplate-preview.png",
    )
