"""Generate tiny original build assets without external image dependencies."""

from pathlib import Path
import struct
import zlib
import zipfile


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def write_icon(path: Path, size: int = 128) -> None:
    rows = []
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            border = x < 6 or y < 6 or x >= size - 6 or y >= size - 6
            display = 18 < x < size - 18 and 24 < y < 67
            accent = 24 < x < 34 and 34 < y < 57
            if border:
                rgba = (12, 18, 20, 255)
            elif accent:
                rgba = (88, 232, 114, 255)
            elif display:
                rgba = (19, 38, 31, 255)
            else:
                rgba = (54, 63, 67, 255)
            row.extend(rgba)
        rows.append(bytes(row))
    raw = b"".join(rows)
    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + png_chunk(b"IHDR", header)
    png += png_chunk(b"IDAT", zlib.compress(raw, 9)) + png_chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)


def bmp(width: int, height: int, pixel) -> bytes:
    padding = (-width * 3) % 4
    rows = bytearray()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            red, green, blue = pixel(x, y)
            rows.extend((blue, green, red))
        rows.extend(b"\0" * padding)
    offset = 14 + 40
    size = offset + len(rows)
    header = b"BM" + struct.pack("<IHHI", size, 0, 0, offset)
    dib = struct.pack("<IiiHHIIiiII", 40, width, height, 1, 24, 0, len(rows), 2835, 2835, 0, 0)
    return header + dib + rows


def chrome_pixel(width: int, height: int):
    def pixel(x: int, y: int):
        if x in (0, width - 1) or y in (0, height - 1):
            return (18, 25, 27)
        if y < 14:
            stripe = 12 if y % 2 else 0
            return (44 + stripe, 58 + stripe, 60 + stripe)
        if 16 < x < width - 17 and 22 < y < min(69, height - 8):
            return (10, 22, 18)
        shade = int(20 * (x / max(1, width - 1)))
        return (68 - shade, 78 - shade, 80 - shade)
    return pixel


def controls_pixel(x: int, y: int):
    pressed = y >= 18
    local_y = y % 18
    boundaries = [0, 23, 46, 69, 92, 114, 136]
    index = next((i for i in range(6) if boundaries[i] <= x < boundaries[i + 1]), 0)
    base = 42 + index * 4 - (12 if pressed else 0)
    if local_y in (0, 17) or x in boundaries:
        return (18, 24, 26)
    return (base + 22, base + 28, base + 30)


def write_default_skin(path: Path) -> None:
    files = {
        "MAIN.BMP": bmp(275, 116, chrome_pixel(275, 116)),
        "EQMAIN.BMP": bmp(275, 116, chrome_pixel(275, 116)),
        "PLEDIT.BMP": bmp(275, 116, chrome_pixel(275, 116)),
        "CBUTTONS.BMP": bmp(136, 36, controls_pixel),
        "PLEDIT.TXT": b"[Text]\r\nNormal=#B4EABF\r\nCurrent=#FFFFFF\r\nNormalBG=#111718\r\nSelectedBG=#344A45\r\n",
        "VISCOLOR.TXT": b"38,90,48\r\n58,150,70\r\n85,223,117\r\n191,255,201\r\n232,233,73\r\n239,110,69\r\n",
        "REGION.TXT": b"[Normal]\r\nNumPoints=4\r\nPointList=0,0,275,0,275,116,0,116\r\n",
        "SKIN.HINTS": b"Model 275 - original Tonelag default skin\r\n",
        "LICENSE.txt": b"Original artwork. SPDX-License-Identifier: MIT OR Apache-2.0\r\n",
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)


if __name__ == "__main__":
    root = Path(__file__).parents[1]
    write_icon(root / "src-tauri" / "icons" / "icon.png")
    write_default_skin(root / "assets" / "default-skin" / "model-275.wsz")
