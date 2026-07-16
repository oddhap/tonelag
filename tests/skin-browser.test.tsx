import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { SkinBrowserPanel } from "../src/components/SkinBrowserPanel";

const backend = vi.hoisted(() => ({
  browseSkinCatalog: vi.fn(),
  installCatalogSkin: vi.fn(),
  onSkinBrowserOpened: vi.fn(),
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
    backend.installCatalogSkin.mockResolvedValue({ id: "a".repeat(64), name: "Receiver", files: ["main.bmp"] });
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
});
