import { useSyncExternalStore } from "react";
import { chooseSkin, getSkinBytes, importSkin, isTauri } from "./backend";
import { parseClassicSkin, type ClassicSkin } from "./skin";

let current: ClassicSkin | null = null;
let loadedId: string | null = null;
const subscribers = new Set<() => void>();

function publish(skin: ClassicSkin | null, id: string | null) {
  current?.dispose();
  current = skin;
  loadedId = id;
  subscribers.forEach((subscriber) => subscriber());
}

export async function loadSkinById(id: string | null) {
  if (!isTauri() || id === loadedId) return;
  if (!id) {
    publish(null, null);
    return;
  }
  const bytes = await getSkinBytes(id);
  publish(await parseClassicSkin(Uint8Array.from(bytes)), id);
}

export async function chooseAndImportSkin() {
  if (!isTauri()) return null;
  const path = await chooseSkin();
  if (!path) return null;
  const descriptor = await importSkin(path);
  const bytes = await getSkinBytes(descriptor.id);
  publish(await parseClassicSkin(Uint8Array.from(bytes), descriptor.name), descriptor.id);
  return descriptor;
}

export function useClassicSkin() {
  return useSyncExternalStore(
    (subscriber) => {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    () => current,
    () => null,
  );
}
