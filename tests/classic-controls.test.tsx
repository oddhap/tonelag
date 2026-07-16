import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { EqualizerPanel } from "../src/components/EqualizerPanel";
import { MainPanel } from "../src/components/MainPanel";
import { defaultSnapshot } from "../src/lib/defaults";

vi.mock("../src/lib/actions", () => ({
  addChosenFiles: vi.fn(), addStreamUrl: vi.fn(), loadEqPreset: vi.fn(), player: vi.fn(),
  reportError: vi.fn(), saveEqPreset: vi.fn(),
}));
vi.mock("../src/lib/backend", () => ({ quitApp: vi.fn(), setPanelVisible: vi.fn() }));
vi.mock("../src/lib/skin-store", () => ({ useClassicSkin: vi.fn(() => null) }));

describe("classic skin controls", () => {
  it("uses sprite-only transport buttons without drawing fallback symbols", () => {
    const { container } = render(<MainPanel snapshot={defaultSnapshot} />);

    const controls = [...container.querySelectorAll(".sprite-button")];
    expect(controls).toHaveLength(6);
    expect(controls.every((control) => control.textContent === "")).toBe(true);
  });

  it("marks equalizer controls for classic sprite and range rendering", () => {
    const { container } = render(<EqualizerPanel snapshot={defaultSnapshot} />);

    expect(container.querySelector(".eq-on-button")).not.toBeNull();
    expect(container.querySelector(".eq-load-button")).not.toBeNull();
    expect(container.querySelector(".eq-save-button")).not.toBeNull();
    expect(container.querySelectorAll(".eq-slider-input")).toHaveLength(11);
  });
});
