import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "../src/i18n";
import { PlaylistPanel } from "../src/components/PlaylistPanel";
import { defaultSnapshot } from "../src/lib/defaults";

vi.mock("../src/lib/actions", () => ({
  addChosenFiles: vi.fn(), addChosenFolder: vi.fn(), clearAll: vi.fn(), player: vi.fn(),
  removeSelected: vi.fn(), reorderItem: vi.fn(), savePlaylist: vi.fn(),
}));
vi.mock("../src/lib/backend", () => ({ setPanelVisible: vi.fn() }));

describe("large playlist rendering", () => {
  it("keeps the mounted row count bounded", () => {
    const queue = Array.from({ length: 5_000 }, (_, index) => ({
      id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
      origin: { kind: "localFile" as const, path: `/music/${index}.mp3` },
      title: `Track ${index}`,
      artist: null,
      durationMs: 180_000,
      available: true,
    }));
    const { container } = render(<PlaylistPanel snapshot={{ ...defaultSnapshot, queue }} />);

    expect(container.querySelectorAll(".playlist-item").length).toBeLessThanOrEqual(80);
  });
});
