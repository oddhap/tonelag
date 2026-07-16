import JSZip from "jszip";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { parseClassicSkin } from "../src/lib/skin";

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
});
