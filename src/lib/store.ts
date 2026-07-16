import { useSyncExternalStore } from "react";
import type { AppSnapshot } from "../bindings/contracts";
import { defaultSnapshot } from "./defaults";
import { getSnapshot, isTauri, onPlaybackSnapshot, onQueueSnapshot, onSnapshot } from "./backend";

let current = defaultSnapshot;
const subscribers = new Set<() => void>();
let started = false;

function publish(snapshot: AppSnapshot) {
  if (snapshot.revision < current.revision) return;
  current = snapshot;
  subscribers.forEach((subscriber) => subscriber());
}

export async function startSnapshotBridge() {
  if (started || !isTauri()) return;
  started = true;
  await onSnapshot(publish);
  await onPlaybackSnapshot((playback) => publish({ ...current, revision: playback.revision, playback }));
  await onQueueSnapshot((queue) => publish({ ...current, queue }));
  publish(await getSnapshot());
}

export function acceptSnapshot(snapshot: AppSnapshot) {
  publish(snapshot);
}

export function useAppSnapshot() {
  return useSyncExternalStore(
    (subscriber) => {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
    () => current,
    () => defaultSnapshot,
  );
}
