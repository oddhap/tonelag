import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSnapshot } from "../bindings/contracts";
import { addChosenFiles, addStreamUrl, player, reportError } from "../lib/actions";
import { quitApp, setPanelVisible } from "../lib/backend";
import { displayTitle, formatTime } from "../lib/format";
import { useClassicSkin } from "../lib/skin-store";
import { PanelChrome } from "./PanelChrome";
import { Spectrum } from "./Spectrum";

interface MainPanelProps {
  snapshot: AppSnapshot;
}

export function MainPanel({ snapshot }: MainPanelProps) {
  const { t } = useTranslation();
  const [showRemaining, setShowRemaining] = useState(false);
  const [visualization, setVisualization] = useState<"spectrum" | "oscilloscope">("spectrum");
  const [menuOpen, setMenuOpen] = useState(false);
  const skin = useClassicSkin();
  const { playback, settings, queue } = snapshot;
  const current = queue.find((item) => item.id === playback.currentItemId) ?? null;
  const title = playback.nowPlayingTitle ?? (current ? displayTitle(current.artist, current.title) : t("noTrack"));
  const remaining = Math.max(0, (playback.durationMs ?? 0) - playback.positionMs);
  const time = playback.capabilities.live
    ? t("live")
    : `${showRemaining && playback.durationMs !== null ? "-" : ""}${formatTime(showRemaining && playback.durationMs !== null ? remaining : playback.positionMs)}`;
  const sourceInfo = [
    playback.bitrateKbps ? `${playback.bitrateKbps} kbps` : null,
    playback.sampleRateHz ? `${(playback.sampleRateHz / 1_000).toFixed(1)} kHz` : null,
    playback.channels ? (playback.channels === 1 ? "MONO" : "STEREO") : null,
  ].filter(Boolean).join(" · ");

  const promptForUrl = () => {
    const value = window.prompt(t("streamUrlPrompt"));
    if (value?.trim()) void addStreamUrl(value.trim());
  };

  const showPanel = (panel: "equalizer" | "playlist" | "skins") => {
    void setPanelVisible(panel, true).catch(reportError);
  };

  const runMenuAction = (action: () => void) => {
    setMenuOpen(false);
    action();
  };

  return (
    <PanelChrome
      className="main-panel"
      title={t("appName")}
      expandedDragArea
      controls={
        <span className="main-title-controls">
          <button
            className="micro-button title-options-button"
            aria-label={t("tonelagMenu")}
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((open) => !open)}
          />
          <button
            className="micro-button title-winshade-button"
            aria-label={t("winshade")}
            aria-pressed={settings.mainWinshade}
            onClick={() => void player({ type: "toggleWinshade" })}
          />
          <button className="micro-button title-close-button" aria-label={t("quit")} onClick={() => void quitApp().catch(reportError)} />
        </span>
      }
    >
      {menuOpen && (
        <div className="tonelag-menu" role="menu" aria-label={t("tonelagMenu")}>
          <button role="menuitem" onClick={() => runMenuAction(() => void addChosenFiles())}>{t("openFiles")}</button>
          <button role="menuitem" onClick={() => runMenuAction(promptForUrl)}>{t("openUrl")}</button>
          <button role="menuitem" onClick={() => runMenuAction(() => showPanel("skins"))}>{t("browseSkins")}</button>
          <button role="menuitem" onClick={() => runMenuAction(() => void player({ type: "toggleAlwaysOnTop" }))}>{t("alwaysOnTop")}{settings.alwaysOnTop ? " ✓" : ""}</button>
          <button role="menuitem" onClick={() => runMenuAction(() => void player({ type: "toggleDoubleSize" }))}>{t("doubleSize")}{settings.doubleSize ? " ✓" : ""}</button>
          <button role="menuitem" onClick={() => runMenuAction(() => void player({ type: "setLanguage", language: settings.language === "en" ? "nb" : "en" }))}>{t("language")}</button>
        </div>
      )}
      <div className="main-readout">
        <button className="time-display" aria-label={showRemaining ? t("remaining") : t("elapsed")} onClick={() => setShowRemaining((value) => !value)}>{time}</button>
        <button className="visualization-toggle" aria-label={t("visualization")} onClick={() => setVisualization((value) => value === "spectrum" ? "oscilloscope" : "spectrum")}>
          <Spectrum values={playback.spectrum} colors={skin?.visualizationColors} mode={visualization} />
        </button>
        <div className="track-marquee"><span>{title}</span></div>
        <span className="source-info">{sourceInfo}</span>
        <span className="stream-state">{playback.status.toUpperCase()}</span>
      </div>

      <input
        className="seek-slider"
        aria-label={t("seek")}
        type="range"
        min={0}
        max={Math.max(1, playback.durationMs ?? 1)}
        value={Math.min(playback.positionMs, playback.durationMs ?? 1)}
        disabled={!playback.capabilities.seekable}
        onChange={(event) => void player({ type: "seek", positionMs: Number(event.currentTarget.value) })}
      />

      <div className="main-controls">
        <div className="transport">
          <button className="sprite-button prev" aria-label={t("previous")} onClick={() => void player({ type: "previous" })} />
          <button className="sprite-button play" aria-label={t("play")} onClick={() => void player({ type: "play" })} />
          <button className="sprite-button pause" aria-label={t("pause")} onClick={() => void player({ type: "pause" })} />
          <button className="sprite-button stop" aria-label={t("stop")} onClick={() => void player({ type: "stop" })} />
          <button className="sprite-button next" aria-label={t("next")} onClick={() => void player({ type: "next" })} />
          <button className="sprite-button eject" aria-label={t("openFiles")} onClick={() => void addChosenFiles()} />
        </div>
        <label className="compact-slider volume-slider" title={t("volume")}>
          <span>VOL</span>
          <input type="range" min={0} max={1} step={0.01} value={settings.volume} onChange={(event) => void player({ type: "setVolume", value: Number(event.currentTarget.value) })} />
        </label>
        <label className="compact-slider balance-slider" title={t("balance")}>
          <span>BAL</span>
          <input type="range" min={-1} max={1} step={0.02} value={settings.balance} onChange={(event) => void player({ type: "setBalance", value: Number(event.currentTarget.value) })} />
        </label>
      </div>

      <div className="main-toggles">
        <button aria-label={t("shuffle")} aria-pressed={settings.shuffle} className={`shuffle-button ${settings.shuffle ? "active" : ""}`} onClick={() => void player({ type: "toggleShuffle" })}>{t("shuffle")}</button>
        <button aria-label={t("repeat")} aria-pressed={settings.repeat} className={`repeat-button ${settings.repeat ? "active" : ""}`} onClick={() => void player({ type: "toggleRepeat" })}>{t("repeat")}</button>
        <button aria-label={t("equalizer")} className="main-eq-button" onClick={() => showPanel("equalizer")}>{t("eq")}</button>
        <button aria-label={t("playlist")} className="main-playlist-button" onClick={() => showPanel("playlist")}>PL</button>
      </div>
      {playback.error && <div className="inline-error" title={playback.error}>! {playback.error}</div>}
    </PanelChrome>
  );
}
