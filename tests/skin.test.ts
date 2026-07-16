import JSZip from "jszip";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { parseClassicSkin, skinCssVariables } from "../src/lib/skin";

function twoPixelBmp(red: number, green: number, blue: number) {
  const bytes = new Uint8Array(62);
  const view = new DataView(bytes.buffer);
  bytes.set([0x42, 0x4d]);
  view.setUint32(2, bytes.length, true);
  view.setUint32(10, 54, true);
  view.setUint32(14, 40, true);
  view.setInt32(18, 2, true);
  view.setInt32(22, 1, true);
  view.setUint16(26, 1, true);
  view.setUint16(28, 24, true);
  view.setUint32(34, 8, true);
  bytes.set([blue, green, red, 0, 0, 0, 0, 0], 54);
  return bytes;
}

describe("classic skin parser", () => {
  beforeEach(() => {
    vi.stubGlobal("URL", { createObjectURL: vi.fn(() => "blob:skin"), revokeObjectURL: vi.fn() });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("accepts a case-insensitive skin under one root", async () => {
    const zip = new JSZip();
    zip.file("RET02/MAIN.BMP", new Uint8Array([0x42, 0x4d]));
    zip.file("RET02/Pledit.txt", "Normal=#00ff00\nNormalBG=#000000");
    const skin = await parseClassicSkin(await zip.generateAsync({ type: "uint8array" }));
    expect(skin.images["main.bmp"]).toBe("blob:skin");
    expect(skin.playlistColors.normal).toBe("#00ff00");
  });

  it("rejects traversal paths", async () => {
    const zip = new JSZip();
    zip.file("../MAIN.BMP", new Uint8Array([0x42, 0x4d]));
    await expect(parseClassicSkin(await zip.generateAsync({ type: "uint8array" }))).rejects.toThrow();
  });

  it("falls back around missing assets and parses regions and animated cursors", async () => {
    const zip = new JSZip();
    zip.file("MAIN.BMP", new Uint8Array([0x42, 0x4d]));
    zip.file("REGION.TXT", "[Normal]\nNumPoints=4\nPointList=0,0,275,0,275,116,0,116");
    zip.file("POINTER.ANI", new Uint8Array([0x52, 0x49, 0x46, 0x46]));
    const skin = await parseClassicSkin(await zip.generateAsync({ type: "uint8array" }));
    expect(skin.images["eqmain.bmp"]).toBeUndefined();
    expect(skin.regionPaths.normal).toContain("M 0 0 L 275 0");
    expect(skin.cursors["pointer.ani"]).toBe("blob:skin");
  });

  it("derives title and time colors from classic skin glyph bitmaps", async () => {
    const zip = new JSZip();
    zip.file("MAIN.BMP", new Uint8Array([0x42, 0x4d]));
    zip.file("TEXT.BMP", twoPixelBmp(204, 51, 34));
    zip.file("NUMBERS.BMP", twoPixelBmp(68, 102, 238));

    const skin = await parseClassicSkin(await zip.generateAsync({ type: "uint8array" }));
    const variables = skinCssVariables(skin) as Record<string, string>;

    expect(variables["--display-text"]).toBe("rgb(204, 51, 34)");
    expect(variables["--time-text"]).toBe("rgb(68, 102, 238)");
  });
});
