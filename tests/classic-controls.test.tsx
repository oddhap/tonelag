import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { EqualizerPanel } from "../src/components/EqualizerPanel";
import { MainPanel } from "../src/components/MainPanel";
import { defaultSnapshot } from "../src/lib/defaults";
import "../src/styles.css";

vi.mock("../src/lib/actions", () => ({
  addChosenFiles: vi.fn(), addStreamUrl: vi.fn(), loadEqPreset: vi.fn(), player: vi.fn(),
  reportError: vi.fn(), saveEqPreset: vi.fn(),
}));
vi.mock("../src/lib/backend", () => ({ quitApp: vi.fn(), setPanelVisible: vi.fn() }));
vi.mock("../src/lib/skin-store", () => ({ useClassicSkin: vi.fn(() => null) }));

describe("classic skin controls", () => {
  afterEach(cleanup);

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

  it("keeps only original title-bar hotspots and moves Tonelag actions into a menu", () => {
    const { container } = render(<MainPanel snapshot={defaultSnapshot} />);

    expect(container.querySelectorAll(".panel-controls button")).toHaveLength(3);
    fireEvent.click(screen.getByRole("button", { name: "Tonelag menu" }));
    expect(screen.getByRole("menuitem", { name: "Browse skins" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Double size" })).toBeInTheDocument();
  });

  it("does not transform the hit-testing layer at double size", () => {
    const rule = [...document.styleSheets]
      .flatMap((sheet) => [...sheet.cssRules])
      .find((candidate) => candidate instanceof CSSStyleRule && candidate.selectorText === ".double-size") as CSSStyleRule | undefined;

    expect(rule?.style.getPropertyValue("transform")).toBe("");
    expect(rule?.style.getPropertyValue("zoom")).toBe("");
  });
});
