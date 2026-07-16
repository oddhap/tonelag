import { useEffect, useState } from "react";
import i18n from "./i18n";
import { MainPanel } from "./components/MainPanel";
import { EqualizerPanel } from "./components/EqualizerPanel";
import { PlaylistPanel } from "./components/PlaylistPanel";
import { SkinBrowserPanel } from "./components/SkinBrowserPanel";
import { addPaths, isTauri, notifyFrontendReady, onDroppedPaths, setInterfaceScale } from "./lib/backend";
import { reportError } from "./lib/actions";
import { loadSkinById, useClassicSkin } from "./lib/skin-store";
import { skinCssClasses, skinCssVariables } from "./lib/skin";
import { acceptSnapshot, useAppSnapshot } from "./lib/store";

type Panel = "main" | "equalizer" | "playlist" | "combined" | "skins";

function requestedPanel(): Panel {
  const panel = new URLSearchParams(window.location.search).get("panel");
  return panel === "equalizer" || panel === "playlist" || panel === "combined" || panel === "skins" ? panel : "main";
}

export default function App() {
  const snapshot = useAppSnapshot();
  const skin = useClassicSkin();
  const [error, setError] = useState<string | null>(null);
  const panel = snapshot.layout.combined && requestedPanel() === "main" ? "combined" : requestedPanel();

  useEffect(() => {
    if (panel === "main" && isTauri()) void notifyFrontendReady().catch(reportError);
  }, [panel]);

  useEffect(() => {
    void i18n.changeLanguage(snapshot.settings.language);
  }, [snapshot.settings.language]);

  useEffect(() => {
    void loadSkinById(snapshot.settings.selectedSkin).catch(reportError);
  }, [snapshot.settings.selectedSkin]);

  useEffect(() => {
    if (!isTauri()) return;
    const scale = panel !== "skins" && snapshot.settings.doubleSize ? 2 : 1;
    void setInterfaceScale(scale).catch(reportError);
  }, [panel, snapshot.settings.doubleSize]);

  useEffect(() => {
    const handler = (event: Event) => {
      setError((event as CustomEvent<string>).detail);
      window.setTimeout(() => setError(null), 5_000);
    };
    window.addEventListener("classic-player-error", handler);
    return () => window.removeEventListener("classic-player-error", handler);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onDroppedPaths((paths) => {
      void addPaths(paths).then(acceptSnapshot).catch(reportError);
    }).then((dispose) => {
      if (disposed) dispose(); else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <main className={`app-shell panel-${panel} ${skin ? "has-imported-skin" : ""} ${skinCssClasses(skin)} ${panel !== "skins" && snapshot.settings.doubleSize ? "double-size" : ""} ${panel !== "skins" && snapshot.settings.mainWinshade ? "winshade" : ""}`} style={skinCssVariables(skin)}>
      {(panel === "main" || panel === "combined") && <MainPanel snapshot={snapshot} />}
      {(panel === "equalizer" || panel === "combined") && <EqualizerPanel snapshot={snapshot} />}
      {(panel === "playlist" || panel === "combined") && <PlaylistPanel snapshot={snapshot} />}
      {panel === "skins" && <SkinBrowserPanel />}
      {error && <aside className="error-toast" role="alert">{error}</aside>}
    </main>
  );
}
