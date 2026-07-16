import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { SkinBrowserPanel } from "../src/components/SkinBrowserPanel";

const backend = vi.hoisted(() => ({
  browseSkinCatalog: vi.fn(),
  deleteInstalledSkin: vi.fn(),
  installCatalogSkin: vi.fn(),
  listInstalledSkins: vi.fn(),
  onSkinBrowserOpened: vi.fn(),
  selectInstalledSkin: vi.fn(),
  setPanelVisible: vi.fn(),
}));

vi.mock("../src/lib/backend", () => ({
  ...backend,
  chooseSkin: vi.fn(async () => null),
  getSkinBytes: vi.fn(),
  importSkin: vi.fn(),
  isTauri: vi.fn(() => true),
}));

describe("skin browser", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    backend.onSkinBrowserOpened.mockImplementation(async (callback: () => void) => {
      queueMicrotask(callback);
      return vi.fn();
    });
    backend.browseSkinCatalog.mockResolvedValue({
      items: [{
        md5: "5eddd4551ab639a85951323a6df7463e",
        name: "Receiver",
        screenshotUrl: "https://r2.webampskins.org/screenshots/5eddd4551ab639a85951323a6df7463e.png",
        museumUrl: "https://skins.webamp.org/skin/5eddd4551ab639a85951323a6df7463e",
      }],
      offset: 0,
      limit: 24,
      totalCount: 1,
      hasMore: false,
    });
    backend.installCatalogSkin.mockResolvedValue({ id: "a".repeat(64), name: "Receiver", files: ["main.bmp"], bundled: false });
    backend.listInstalledSkins.mockResolvedValue([]);
    backend.selectInstalledSkin.mockResolvedValue({
      revision: 1,
      queue: [],
      playback: { revision: 1 },
      settings: { selectedSkin: null },
      layout: {},
    });
  });

  it("browses and installs a catalog skin", async () => {
    render(<SkinBrowserPanel />);

    expect(await screen.findByText("Receiver")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Install" }));

    await waitFor(() => expect(backend.installCatalogSkin).toHaveBeenCalledWith(
      "5eddd4551ab639a85951323a6df7463e",
      "Receiver",
    ));
    expect(await screen.findByRole("button", { name: "Installed" })).toBeDisabled();
  });

  it("sends a trimmed search to the native catalog client", async () => {
    render(<SkinBrowserPanel />);
    await screen.findByText("Receiver");

    fireEvent.change(screen.getByLabelText("Search classic skins"), { target: { value: "  zelda  " } });
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    await waitFor(() => expect(backend.browseSkinCatalog).toHaveBeenLastCalledWith("zelda", 0, 24));
  });

  it("shows and selects a locally installed skin", async () => {
    backend.listInstalledSkins.mockResolvedValue([
      { id: "b".repeat(64), name: "Local receiver", files: ["main.bmp"], bundled: false },
    ]);
    render(<SkinBrowserPanel />);

    fireEvent.change(await screen.findByRole("combobox", { name: "Installed skins" }), {
      target: { value: "b".repeat(64) },
    });
    fireEvent.click(screen.getByRole("button", { name: "Use selected skin" }));

    expect(backend.selectInstalledSkin).toHaveBeenCalledWith("b".repeat(64));
  });

  it("deletes a removable local skin without laying skins out horizontally", async () => {
    backend.listInstalledSkins.mockResolvedValue([
      { id: "d".repeat(64), name: "Delete me", files: ["main.bmp"], bundled: false },
    ]);
    backend.deleteInstalledSkin.mockResolvedValue({
      revision: 2, queue: [], playback: { revision: 2 }, settings: { selectedSkin: null }, layout: {},
    });
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<SkinBrowserPanel />);

    fireEvent.change(await screen.findByRole("combobox", { name: "Installed skins" }), {
      target: { value: "d".repeat(64) },
    });
    backend.listInstalledSkins.mockResolvedValue([]);
    fireEvent.click(screen.getByRole("button", { name: "Delete selected skin" }));

    await waitFor(() => expect(backend.deleteInstalledSkin).toHaveBeenCalledWith("d".repeat(64)));
  });

  it("loads installed skins even when the initial opened event was emitted too early", async () => {
    backend.onSkinBrowserOpened.mockResolvedValue(vi.fn());
    backend.listInstalledSkins.mockResolvedValue([
      { id: "c".repeat(64), name: "Already installed", files: ["main.bmp"], bundled: false },
    ]);

    render(<SkinBrowserPanel />);

    expect(await screen.findByRole("option", { name: "Already installed" })).toBeInTheDocument();
  });
});
