import { useTranslation } from "react-i18next";
import type { AppSnapshot } from "../bindings/contracts";
import { loadEqPreset, player, saveEqPreset } from "../lib/actions";
import { setPanelVisible } from "../lib/backend";
import { PanelChrome } from "./PanelChrome";

const frequencies = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];

export function EqualizerPanel({ snapshot }: { snapshot: AppSnapshot }) {
  const { t } = useTranslation();
  const eq = snapshot.settings.eq;
  return (
    <PanelChrome
      className="equalizer-panel"
      title={t("equalizer")}
      controls={<button className="micro-button" aria-label={t("close")} onClick={() => void setPanelVisible("equalizer", false)}>×</button>}
    >
      <div className="eq-toolbar">
        <button className={eq.enabled ? "active" : ""} onClick={() => void player({ type: "setEqEnabled", enabled: !eq.enabled })}>
          {eq.enabled ? t("enabled") : t("disabled")}
        </button>
        <button onClick={() => void loadEqPreset()}>LOAD</button>
        <button onClick={() => void saveEqPreset()}>SAVE</button>
        <span className="eq-curve" aria-hidden="true">
          {[eq.preampDb, ...eq.bandsDb].map((value, index) => <i key={index} style={{ transform: `translateY(${-value * 0.3}px)` }} />)}
        </span>
      </div>
      <div className="eq-sliders">
        <EqSlider label={t("preamp")} value={eq.preampDb} onChange={(value) => player({ type: "setPreamp", valueDb: value })} />
        {eq.bandsDb.map((value, index) => (
          <EqSlider key={frequencies[index]} label={frequencies[index] ?? ""} value={value} onChange={(valueDb) => player({ type: "setEqBand", index, valueDb })} />
        ))}
      </div>
    </PanelChrome>
  );
}

function EqSlider({ label, value, onChange }: { label: string; value: number; onChange(value: number): void }) {
  return (
    <label className="eq-slider" title={`${label}: ${value.toFixed(1)} dB`}>
      <input type="range" min={-12} max={12} step={0.5} value={value} onChange={(event) => onChange(Number(event.currentTarget.value))} />
      <span>{label}</span>
    </label>
  );
}
