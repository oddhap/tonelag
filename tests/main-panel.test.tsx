import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { defaultSnapshot } from "../src/lib/defaults";
import { MainPanel } from "../src/components/MainPanel";

const backend = vi.hoisted(() => ({ quitApp: vi.fn(async () => undefined) }));
vi.mock("../src/lib/backend", () => ({ ...backend, setPanelVisible: vi.fn(async () => undefined) }));
vi.mock("../src/lib/actions", () => ({
  addChosenFiles: vi.fn(), addStreamUrl: vi.fn(), player: vi.fn(), reportError: vi.fn(),
}));
vi.mock("../src/lib/skin-store", () => ({ useClassicSkin: vi.fn(() => null) }));

describe("main panel", () => {
  it("exposes an application quit button", () => {
    render(<MainPanel snapshot={defaultSnapshot} />);

    fireEvent.click(screen.getByRole("button", { name: "Quit Tonelag" }));

    expect(backend.quitApp).toHaveBeenCalledOnce();
  });
});
