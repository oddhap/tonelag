import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSnapshot } from "../bindings/contracts";
import { addChosenFiles, addStreamUrl, player, reportError } from "../lib/actions";
import { setPanelVisible } from "../lib/backend";
import { displayTitle, formatTime } from "../lib/format";
import { chooseAndImportSkin, useClassicSkin } from "../lib/skin-store";
import { PanelChrome } from "./PanelChrome";
import { Spectrum } from "./Spectrum";

interface MainPanelProps {
  snapshot: AppSnapshot;
}

export function MainPanel({ snapshot }: MainPanelProps) {
  const { t } = useTranslation();
  const [showRemaining, setShowRemaining] = useState(false);
  const [visualization, setVisualization] = useState<"spectrum" | "oscilloscope">("spectrum");
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

  const showPanel = (panel: "equalizer" | "playlist") => {
    void setPanelVisible(panel, true).catch(reportError);
  };

  return (
    <PanelChrome
      className="main-panel"
      title={t("appName")}
      controls={
        <>
          <button className="micro-button" onClick={() => void chooseAndImportSkin().catch(reportError)} title={t("importSkin")}>S</button>
          <button className="micro-button" onClick={() => void addChosenFiles()} title={t("openFiles")}>O</button>
          <button className="micro-button" onClick={promptForUrl} title={t("openUrl")}>U</button>
          <button className="micro-button" onClick={() => void player({ type: "setLanguage", language: settings.language === "en" ? "nb" : "en" })} title={t("language")}>{settings.language === "en" ? "N" : "E"}</button>
          <button className="micro-button" aria-pressed={settings.alwaysOnTop} onClick={() => void player({ type: "toggleAlwaysOnTop" })} title={t("alwaysOnTop")}>A</button>
          <button className="micro-button" aria-pressed={settings.doubleSize} onClick={() => void player({ type: "toggleDoubleSize" })} title="2×">2</button>
          <button className="micro-button" aria-pressed={settings.mainWinshade} onClick={() => void player({ type: "toggleWinshade" })} title="Winshade">W</button>
        </>
      }
    >
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
          <button className="sprite-button prev" aria-label={t("previous")} onClick={() => void player({ type: "previous" })}>◀◀</button>
          <button className="sprite-button play" aria-label={t("play")} onClick={() => void player({ type: "play" })}>▶</button>
          <button className="sprite-button pause" aria-label={t("pause")} onClick={() => void player({ type: "pause" })}>Ⅱ</button>
          <button className="sprite-button stop" aria-label={t("stop")} onClick={() => void player({ type: "stop" })}>■</button>
          <button className="sprite-button next" aria-label={t("next")} onClick={() => void player({ type: "next" })}>▶▶</button>
          <button className="sprite-button eject" aria-label={t("openFiles")} onClick={() => void addChosenFiles()}>▲</button>
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
        <button aria-pressed={settings.shuffle} className={settings.shuffle ? "active" : ""} onClick={() => void player({ type: "toggleShuffle" })}>{t("shuffle")}</button>
        <button aria-pressed={settings.repeat} className={settings.repeat ? "active" : ""} onClick={() => void player({ type: "toggleRepeat" })}>{t("repeat")}</button>
        <button onClick={() => showPanel("equalizer")}>{t("eq")}</button>
        <button onClick={() => showPanel("playlist")}>PL</button>
      </div>
      {playback.error && <div className="inline-error" title={playback.error}>! {playback.error}</div>}
    </PanelChrome>
  );
}
