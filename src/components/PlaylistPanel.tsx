import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSnapshot } from "../bindings/contracts";
import { addChosenFiles, addChosenFolder, clearAll, player, removeSelected, reorderItem, savePlaylist } from "../lib/actions";
import { setPanelVisible } from "../lib/backend";
import { displayTitle, formatTime } from "../lib/format";
import { PanelChrome } from "./PanelChrome";

export function PlaylistPanel({ snapshot }: { snapshot: AppSnapshot }) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const listRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState({ top: 0, height: 232 });
  const rowHeight = 16;
  const overscan = 10;
  const firstVisible = Math.max(0, Math.floor(viewport.top / rowHeight) - overscan);
  const visibleCount = Math.ceil(viewport.height / rowHeight) + overscan * 2;
  const lastVisible = Math.min(snapshot.queue.length, firstVisible + visibleCount);
  const visibleItems = snapshot.queue.slice(firstVisible, lastVisible);
  const totalDuration = useMemo(
    () => snapshot.queue.reduce((total, item) => total + (item.durationMs ?? 0), 0),
    [snapshot.queue],
  );

  useEffect(() => {
    const list = listRef.current;
    if (!list || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setViewport((current) => ({ ...current, height: entry.contentRect.height }));
    });
    observer.observe(list);
    return () => observer.disconnect();
  }, []);

  const select = (id: string, additive: boolean) => {
    setSelected((previous) => {
      const next = additive ? new Set(previous) : new Set<string>();
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Delete" || event.key === "Backspace") {
      if (selected.size) {
        event.preventDefault();
        void removeSelected([...selected]);
        setSelected(new Set());
      }
      return;
    }
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    const direction = event.key === "ArrowUp" ? -1 : 1;
    const selectedId = selected.values().next().value as string | undefined;
    const current = Math.max(0, snapshot.queue.findIndex((item) => item.id === selectedId));
    const target = Math.max(0, Math.min(snapshot.queue.length - 1, current + direction));
    if (event.altKey && selected.size === 1) {
      void reorderItem(current, target);
    } else if (snapshot.queue[target]) {
      setSelected(new Set([snapshot.queue[target].id]));
      const list = listRef.current;
      if (list && (target < firstVisible || target >= lastVisible)) {
        list.scrollTop = target * rowHeight;
        setViewport({ top: list.scrollTop, height: list.clientHeight || viewport.height });
      }
    }
  };

  const cropSelection = () => {
    const remove = snapshot.queue.filter((item) => !selected.has(item.id)).map((item) => item.id);
    void removeSelected(remove);
  };

  return (
    <PanelChrome
      className="playlist-panel"
      title={`${t("playlist")} · ${snapshot.queue.length}`}
      controls={<button className="micro-button" aria-label={t("close")} onClick={() => void setPanelVisible("playlist", false)}>×</button>}
    >
      <div
        className="playlist-list"
        role="listbox"
        aria-multiselectable="true"
        tabIndex={0}
        ref={listRef}
        onScroll={(event) => setViewport({ top: event.currentTarget.scrollTop, height: event.currentTarget.clientHeight || viewport.height })}
        onKeyDown={handleKeyDown}
      >
        {snapshot.queue.length === 0 && <p className="playlist-empty">{t("emptyPlaylist")}</p>}
        <div className="playlist-scroll-space" style={{ height: snapshot.queue.length * rowHeight }}>
        {visibleItems.map((item, visibleIndex) => {
          const index = firstVisible + visibleIndex;
          const active = item.id === snapshot.playback.currentItemId;
          const isSelected = selected.has(item.id);
          return (
            <button
              key={item.id}
              role="option"
              aria-selected={isSelected}
              draggable
              className={`playlist-item ${active ? "current" : ""} ${!item.available ? "unavailable" : ""}`}
              style={{ top: index * rowHeight }}
              onClick={(event) => select(item.id, event.metaKey || event.ctrlKey)}
              onDoubleClick={() => void player({ type: "load", itemId: item.id })}
              onDragStart={(event) => event.dataTransfer.setData("application/x-classic-queue-index", String(index))}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                event.preventDefault();
                const from = Number(event.dataTransfer.getData("application/x-classic-queue-index"));
                if (Number.isInteger(from)) void reorderItem(from, index);
              }}
            >
              <span className="playlist-index">{String(index + 1).padStart(2, "0")}.</span>
              <span className="playlist-title">{displayTitle(item.artist, item.title)}</span>
              <time>{formatTime(item.durationMs)}</time>
            </button>
          );
        })}
        </div>
      </div>
      <footer className="playlist-footer">
        <div className="playlist-menu">
          <button className="playlist-add-button" onClick={() => void addChosenFiles()}>{t("add")}</button>
          <button className="playlist-dir-button" onClick={() => void addChosenFolder()}>DIR</button>
          <button className="playlist-remove-button" disabled={!selected.size} onClick={() => { void removeSelected([...selected]); setSelected(new Set()); }}>{t("remove")}</button>
          <button className="playlist-crop-button" disabled={!selected.size} onClick={cropSelection}>{t("crop")}</button>
          <button className="playlist-clear-button" disabled={!snapshot.queue.length} onClick={() => void clearAll()}>{t("clear")}</button>
          <button className="playlist-save-button" disabled={!snapshot.queue.length} onClick={() => void savePlaylist()}>{t("save")}</button>
        </div>
        <output>{formatTime(totalDuration)}</output>
      </footer>
    </PanelChrome>
  );
}
