import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppSnapshot,
  PlayerCommand,
  ResolvedStream,
  SkinCatalogPage,
  SkinDescriptor,
  Uuid,
} from "../bindings/contracts";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export const getSnapshot = () => invoke<AppSnapshot>("get_snapshot");
export const sendPlayerCommand = (command: PlayerCommand) =>
  invoke<AppSnapshot>("player_command", { command });
export const addPaths = (paths: string[]) =>
  invoke<AppSnapshot>("queue_add_paths", { paths });
export const addUrl = (url: string, title: string | null = null) => invoke<AppSnapshot>("queue_add_url", { url, title });
export const removeItems = (ids: Uuid[]) => invoke<AppSnapshot>("queue_remove", { ids });
export const clearQueue = () => invoke<AppSnapshot>("queue_clear");
export const reorderQueue = (from: number, to: number) =>
  invoke<AppSnapshot>("queue_reorder", { from, to });
export const exportPlaylist = (path: string) => invoke<void>("playlist_export", { path });
export const importEqf = (path: string) => invoke<AppSnapshot>("eqf_import", { path });
export const exportEqf = (path: string) => invoke<void>("eqf_export", { path });
export const importSkin = (path: string) => invoke<SkinDescriptor>("skin_import", { path });
export const getSkinBytes = (id: string) => invoke<number[]>("skin_bytes", { id });
export const browseSkinCatalog = (query: string | null, offset: number, limit: number) =>
  invoke<SkinCatalogPage>("skin_catalog_browse", { query, offset, limit });
export const installCatalogSkin = (md5: string, name: string) =>
  invoke<SkinDescriptor>("skin_catalog_install", { md5, name });
export const resolveStream = (url: string) => invoke<ResolvedStream>("resolve_stream", { url });
export const setPanelVisible = (panel: "equalizer" | "playlist" | "skins", visible: boolean) =>
  invoke<void>("set_panel_visible", { panel, visible });

export async function chooseAudioFiles(): Promise<string[]> {
  const result = await open({
    multiple: true,
    filters: [
      {
        name: "Audio and playlists",
        extensions: [
          "mp2", "mp3", "aac", "m4a", "flac", "alac", "wma", "ape", "wv", "mpc",
          "ogg", "opus", "wav", "aiff", "mod", "xm", "s3m", "it", "m3u", "m3u8", "pls",
        ],
      },
    ],
  });
  if (!result) return [];
  return Array.isArray(result) ? result : [result];
}

export async function chooseFolder(): Promise<string[]> {
  const result = await open({ directory: true, multiple: false });
  return result ? [result] : [];
}

export async function chooseSkin(): Promise<string | null> {
  const result = await open({
    multiple: false,
    filters: [{ name: "Classic skins", extensions: ["wsz", "zip"] }],
  });
  return Array.isArray(result) ? result[0] ?? null : result;
}

export const choosePlaylistDestination = () =>
  save({ defaultPath: "playlist.m3u8", filters: [{ name: "Playlists", extensions: ["m3u8", "m3u", "pls"] }] });

export async function chooseEqf(): Promise<string | null> {
  const result = await open({
    multiple: false,
    filters: [{ name: "Equalizer presets", extensions: ["eqf"] }],
  });
  return Array.isArray(result) ? result[0] ?? null : result;
}

export const chooseEqfDestination = () =>
  save({ defaultPath: "preset.eqf", filters: [{ name: "Equalizer presets", extensions: ["eqf"] }] });

export const onSnapshot = (callback: (snapshot: AppSnapshot) => void) =>
  listen<AppSnapshot>("app://snapshot", ({ payload }) => callback(payload));
export const onSkinBrowserOpened = (callback: () => void) =>
  listen<void>("skin-browser://opened", callback);

export async function onDroppedPaths(callback: (paths: string[]) => void) {
  return getCurrentWebviewWindow().onDragDropEvent((event) => {
    if (event.payload.type === "drop") callback(event.payload.paths);
  });
}
