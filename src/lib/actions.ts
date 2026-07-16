import type { PlayerCommand } from "../bindings/contracts";
import {
  addPaths,
  addUrl,
  chooseAudioFiles,
  chooseFolder,
  chooseEqf,
  chooseEqfDestination,
  choosePlaylistDestination,
  clearQueue,
  exportPlaylist,
  exportEqf,
  importEqf,
  isTauri,
  removeItems,
  reorderQueue,
  resolveStream,
  sendPlayerCommand,
} from "./backend";
import { acceptSnapshot } from "./store";

export const reportError = (error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  window.dispatchEvent(new CustomEvent("classic-player-error", { detail: message }));
};

export async function player(command: PlayerCommand) {
  if (!isTauri()) return;
  try {
    acceptSnapshot(await sendPlayerCommand(command));
  } catch (error) {
    reportError(error);
  }
}

export async function addChosenFiles() {
  try {
    const paths = await chooseAudioFiles();
    if (paths.length) acceptSnapshot(await addPaths(paths));
  } catch (error) {
    reportError(error);
  }
}

export async function addChosenFolder() {
  try {
    const paths = await chooseFolder();
    if (paths.length) acceptSnapshot(await addPaths(paths));
  } catch (error) {
    reportError(error);
  }
}

export async function addStreamUrl(url: string) {
  try {
    const stream = await resolveStream(url);
    acceptSnapshot(await addUrl(stream.url, stream.stationName));
  } catch (error) {
    reportError(error);
  }
}

export async function removeSelected(ids: string[]) {
  try {
    acceptSnapshot(await removeItems(ids));
  } catch (error) {
    reportError(error);
  }
}

export async function reorderItem(from: number, to: number) {
  try {
    acceptSnapshot(await reorderQueue(from, to));
  } catch (error) {
    reportError(error);
  }
}

export async function clearAll() {
  try {
    acceptSnapshot(await clearQueue());
  } catch (error) {
    reportError(error);
  }
}

export async function savePlaylist() {
  try {
    const path = await choosePlaylistDestination();
    if (path) await exportPlaylist(path);
  } catch (error) {
    reportError(error);
  }
}

export async function loadEqPreset() {
  try {
    const path = await chooseEqf();
    if (path) acceptSnapshot(await importEqf(path));
  } catch (error) {
    reportError(error);
  }
}

export async function saveEqPreset() {
  try {
    const path = await chooseEqfDestination();
    if (path) await exportEqf(path);
  } catch (error) {
    reportError(error);
  }
}
