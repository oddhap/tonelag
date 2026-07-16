import JSZip, { type JSZipObject } from "jszip";
import type { CSSProperties } from "react";

const MAX_FILES = 1_000;
const MAX_EXPANDED_BYTES = 100 * 1024 * 1024;

const IMAGE_NAMES = [
  "main.bmp", "cbuttons.bmp", "titlebar.bmp", "shufrep.bmp", "monoster.bmp", "playpaus.bmp",
  "text.bmp", "numbers.bmp", "nums_ex.bmp", "posbar.bmp", "volume.bmp", "balance.bmp",
  "eqmain.bmp", "eq_ex.bmp", "pledit.bmp", "gen.bmp", "genex.bmp",
] as const;

export interface ClassicSkin {
  name: string;
  images: Record<string, string>;
  playlistColors: Record<string, string>;
  visualizationColors: string[];
  regions: string | null;
  regionPaths: Record<string, string>;
  hints: string | null;
  genericColors: Record<string, string>;
  cursors: Record<string, string>;
  glyphColors: { text: string | null; numbers: string | null };
  dispose(): void;
}

const safePath = (path: string) => {
  const normalized = path.replaceAll("\\", "/").replace(/^\.\//, "");
  const pieces = normalized.split("/");
  if (normalized.startsWith("/") || pieces.some((piece) => piece === "..")) {
    throw new Error("Skin contains an unsafe archive path");
  }
  return normalized;
};

function stripSingleRoot(paths: string[]): Map<string, string> {
  const safe = paths.map(safePath);
  const files = safe.filter((path) => path && !path.endsWith("/"));
  const roots = new Set(files.filter((path) => path.includes("/")).map((path) => path.split("/")[0]?.toLowerCase()));
  const hasRootFiles = files.some((path) => !path.includes("/"));
  const strip = !hasRootFiles && roots.size === 1;
  const result = new Map<string, string>();
  for (const path of files) {
    const normalized = strip ? path.slice(path.indexOf("/") + 1) : path;
    if (normalized.includes("/")) continue;
    result.set(normalized.toLowerCase(), path);
  }
  return result;
}

function parseKeyValue(text: string): Record<string, string> {
  const values: Record<string, string> = {};
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("[") || line.startsWith(";") || line.startsWith("#")) continue;
    const separator = line.indexOf("=");
    if (separator < 1) continue;
    values[line.slice(0, separator).trim().toLowerCase()] = line.slice(separator + 1).trim();
  }
  return values;
}

function parseRegionPaths(text: string | null): Record<string, string> {
  if (!text) return {};
  const sections: Record<string, Record<string, string>> = {};
  let section = "normal";
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    const heading = line.match(/^\[([^\]]+)]$/);
    if (heading?.[1]) {
      section = heading[1].toLowerCase();
      continue;
    }
    const [key, value] = line.split("=", 2);
    if (!key || value === undefined) continue;
    (sections[section] ??= {})[key.trim().toLowerCase()] = value.trim();
  }

  const paths: Record<string, string> = {};
  for (const [name, values] of Object.entries(sections)) {
    const pointCounts = (values.numpoints ?? "").split(",").map(Number).filter((value) => value > 0);
    const coordinates = (values.pointlist ?? "").split(",").map(Number).filter(Number.isFinite);
    if (!pointCounts.length || coordinates.length < 6) continue;
    let coordinateIndex = 0;
    const segments: string[] = [];
    for (const count of pointCounts) {
      const points: string[] = [];
      for (let index = 0; index < count; index += 1) {
        const x = coordinates[coordinateIndex++];
        const y = coordinates[coordinateIndex++];
        if (x === undefined || y === undefined) break;
        points.push(`${x} ${y}`);
      }
      if (points.length >= 3) segments.push(`M ${points.join(" L ")} Z`);
    }
    if (segments.length) paths[name] = `path("${segments.join(" ")}")`;
  }
  return paths;
}

async function optionalText(zip: JSZip, files: Map<string, string>, name: string) {
  const path = files.get(name);
  return path ? zip.file(path)?.async("text") ?? null : null;
}

async function objectUrl(entry: JSZipObject, type: string) {
  const bytes = await entry.async("uint8array");
  const buffer = Uint8Array.from(bytes).buffer;
  return { url: URL.createObjectURL(new Blob([buffer], { type })), bytes };
}

function bitmapPixels(bytes: Uint8Array): Array<[number, number, number]> {
  if (bytes.length < 54 || bytes[0] !== 0x42 || bytes[1] !== 0x4d) return [];
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const pixelOffset = view.getUint32(10, true);
  const dibSize = view.getUint32(14, true);
  const width = Math.abs(view.getInt32(18, true));
  const height = Math.abs(view.getInt32(22, true));
  const bits = view.getUint16(28, true);
  const compression = view.getUint32(30, true);
  if (!width || !height || compression !== 0 || ![1, 4, 8, 24, 32].includes(bits)) return [];
  const rowStride = Math.floor((width * bits + 31) / 32) * 4;
  if (pixelOffset + rowStride * height > bytes.length) return [];

  const palette: Array<[number, number, number]> = [];
  if (bits <= 8) {
    const paletteStart = 14 + dibSize;
    const declared = bytes.length >= 50 ? view.getUint32(46, true) : 0;
    const count = declared || 1 << bits;
    if (paletteStart + count * 4 > pixelOffset) return [];
    for (let index = 0; index < count; index += 1) {
      const offset = paletteStart + index * 4;
      palette.push([bytes[offset + 2] ?? 0, bytes[offset + 1] ?? 0, bytes[offset] ?? 0]);
    }
  }

  const pixels: Array<[number, number, number]> = [];
  for (let y = 0; y < height; y += 1) {
    const row = pixelOffset + y * rowStride;
    for (let x = 0; x < width; x += 1) {
      if (bits === 24 || bits === 32) {
        const offset = row + x * (bits / 8);
        pixels.push([bytes[offset + 2] ?? 0, bytes[offset + 1] ?? 0, bytes[offset] ?? 0]);
      } else {
        const packed = bytes[row + Math.floor(x * bits / 8)] ?? 0;
        const shift = 8 - bits - (x * bits % 8);
        const index = (packed >> shift) & ((1 << bits) - 1);
        if (palette[index]) pixels.push(palette[index]);
      }
    }
  }
  return pixels;
}

function representativeBitmapColor(bytes: Uint8Array): string | null {
  const histogram = new Map<string, { color: [number, number, number]; count: number }>();
  for (const color of bitmapPixels(bytes)) {
    const key = color.join(",");
    const entry = histogram.get(key);
    if (entry) entry.count += 1;
    else histogram.set(key, { color, count: 1 });
  }
  const colors = [...histogram.values()];
  if (!colors.length) return null;
  const background = colors.reduce((best, entry) => {
    if (entry.count !== best.count) return entry.count > best.count ? entry : best;
    const luminance = entry.color[0] + entry.color[1] + entry.color[2];
    const bestLuminance = best.color[0] + best.color[1] + best.color[2];
    return luminance < bestLuminance ? entry : best;
  });
  const foreground = colors
    .filter((entry) => entry !== background)
    .map((entry) => ({
      ...entry,
      score: entry.color.reduce((sum, channel, index) => sum + (channel - background.color[index]) ** 2, 0) * Math.sqrt(entry.count),
    }))
    .sort((left, right) => right.score - left.score)[0] ?? background;
  return `rgb(${foreground.color.join(", ")})`;
}

export async function parseClassicSkin(bytes: Uint8Array, name = "Imported skin"): Promise<ClassicSkin> {
  const zip = await JSZip.loadAsync(bytes, { createFolders: false, checkCRC32: true });
  const entries = Object.values(zip.files);
  if (entries.length > MAX_FILES) throw new Error("Skin contains more than 1000 files");

  let expanded = 0;
  for (const entry of entries) {
    if (entry.dir) continue;
    const data = await entry.async("uint8array");
    expanded += data.byteLength;
    if (expanded > MAX_EXPANDED_BYTES) throw new Error("Skin exceeds the 100 MiB expanded limit");
  }

  const files = stripSingleRoot(entries.map((entry) => entry.name));
  if (!files.has("main.bmp")) throw new Error("Skin does not contain MAIN.BMP");

  const urls: string[] = [];
  const images: Record<string, string> = {};
  const glyphColors = { text: null as string | null, numbers: null as string | null };
  for (const imageName of IMAGE_NAMES) {
    const path = files.get(imageName);
    const entry = path ? zip.file(path) : null;
    if (!entry) continue;
    const image = await objectUrl(entry, "image/bmp");
    urls.push(image.url);
    images[imageName] = image.url;
    if (imageName === "text.bmp") glyphColors.text = representativeBitmapColor(image.bytes);
    if (imageName === "numbers.bmp" || imageName === "nums_ex.bmp") {
      glyphColors.numbers ??= representativeBitmapColor(image.bytes);
    }
  }

  const cursors: Record<string, string> = {};
  for (const [leaf, path] of files) {
    if (!leaf.endsWith(".cur") && !leaf.endsWith(".ani")) continue;
    const entry = zip.file(path);
    if (!entry) continue;
    const image = await objectUrl(entry, leaf.endsWith(".ani") ? "application/x-navi-animation" : "image/x-icon");
    urls.push(image.url);
    cursors[leaf] = image.url;
  }

  const playlistText = await optionalText(zip, files, "pledit.txt");
  const visText = await optionalText(zip, files, "viscolor.txt");
  const regions = await optionalText(zip, files, "region.txt");
  const genericColors = await optionalText(zip, files, "genex.cols");
  return {
    name,
    images,
    playlistColors: playlistText ? parseKeyValue(playlistText) : {},
    visualizationColors: (visText ?? "").split(/\r?\n/).map((line) => line.trim()).filter((line) => /^\d+\s*,\s*\d+\s*,\s*\d+$/.test(line)).map((line) => `rgb(${line})`),
    regions,
    regionPaths: parseRegionPaths(regions),
    hints: await optionalText(zip, files, "skin.hints"),
    genericColors: genericColors ? parseKeyValue(genericColors) : {},
    cursors,
    glyphColors,
    dispose: () => urls.forEach((url) => URL.revokeObjectURL(url)),
  };
}

export function skinCssVariables(skin: ClassicSkin | null): CSSProperties {
  if (!skin) return {};
  const variables: Record<string, string> = {};
  for (const [name, value] of Object.entries(skin.images)) {
    variables[`--skin-${name.replace(".bmp", "").replaceAll("_", "-")}`] = `url("${value}")`;
  }
  const colors = skin.playlistColors;
  if (colors.normal) variables["--playlist-text"] = colors.normal;
  if (colors.current) variables["--playlist-current"] = colors.current;
  if (colors.normalbg) variables["--playlist-bg"] = colors.normalbg;
  if (colors.selectedbg) variables["--playlist-selected"] = colors.selectedbg;
  if (skin.glyphColors.text) variables["--display-text"] = skin.glyphColors.text;
  if (skin.glyphColors.numbers) variables["--time-text"] = skin.glyphColors.numbers;
  if (skin.regionPaths.normal) variables["--skin-main-clip"] = skin.regionPaths.normal;
  if (skin.regionPaths.equalizer) variables["--skin-eq-clip"] = skin.regionPaths.equalizer;
  const cursor = Object.entries(skin.cursors).find(([name]) => name.endsWith(".cur"))?.[1]
    ?? Object.entries(skin.cursors).find(([name]) => name.endsWith(".ani"))?.[1];
  if (cursor) variables["--skin-cursor"] = `url("${cursor}"), default`;
  return variables as CSSProperties;
}

export function skinCssClasses(skin: ClassicSkin | null): string {
  if (!skin) return "";
  return Object.keys(skin.images)
    .map((name) => `has-skin-${name.replace(".bmp", "").replaceAll("_", "-")}`)
    .join(" ");
}
