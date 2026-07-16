import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSnapshot } from "../bindings/contracts";
import { addChosenFiles, addChosenFolder, clearAll, player, removeSelected, reorderItem, savePlaylist } from "../lib/actions";
import { setPanelVisible } from "../lib/backend";
import { displayTitle, formatTime } from "../lib/format";
import { PanelChrome } from "./PanelChrome";

export function PlaylistPanel({ snapshot }: { snapshot: AppSnapshot }) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(new Set());

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
      <div className="playlist-list" role="listbox" aria-multiselectable="true" tabIndex={0} onKeyDown={handleKeyDown}>
        {snapshot.queue.length === 0 && <p className="playlist-empty">{t("emptyPlaylist")}</p>}
        {snapshot.queue.map((item, index) => {
          const active = item.id === snapshot.playback.currentItemId;
          const isSelected = selected.has(item.id);
          return (
            <button
              key={item.id}
              role="option"
              aria-selected={isSelected}
              draggable
              className={`playlist-item ${active ? "current" : ""} ${!item.available ? "unavailable" : ""}`}
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
      <footer className="playlist-footer">
        <div className="playlist-menu">
          <button onClick={() => void addChosenFiles()}>{t("add")}</button>
          <button onClick={() => void addChosenFolder()}>DIR</button>
          <button disabled={!selected.size} onClick={() => { void removeSelected([...selected]); setSelected(new Set()); }}>{t("remove")}</button>
          <button disabled={!selected.size} onClick={cropSelection}>{t("crop")}</button>
          <button disabled={!snapshot.queue.length} onClick={() => void clearAll()}>{t("clear")}</button>
          <button disabled={!snapshot.queue.length} onClick={() => void savePlaylist()}>{t("save")}</button>
        </div>
        <output>{formatTime(snapshot.queue.reduce((total, item) => total + (item.durationMs ?? 0), 0))}</output>
      </footer>
    </PanelChrome>
  );
}
