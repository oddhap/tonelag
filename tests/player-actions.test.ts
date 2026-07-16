import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  isTauri: vi.fn(() => true),
  sendPlayerCommand: vi.fn(),
}));
const store = vi.hoisted(() => ({ acceptSnapshot: vi.fn() }));

vi.mock("../src/lib/backend", () => ({
  ...backend,
  addPaths: vi.fn(), addUrl: vi.fn(), chooseAudioFiles: vi.fn(), chooseFolder: vi.fn(),
  chooseEqf: vi.fn(), chooseEqfDestination: vi.fn(), choosePlaylistDestination: vi.fn(),
  clearQueue: vi.fn(), exportPlaylist: vi.fn(), exportEqf: vi.fn(), importEqf: vi.fn(),
  removeItems: vi.fn(), reorderQueue: vi.fn(), resolveStream: vi.fn(),
}));
vi.mock("../src/lib/store", () => store);

import { player } from "../src/lib/actions";

describe("player actions", () => {
  beforeEach(() => vi.clearAllMocks());

  it("does not replace the queue with a lightweight player-command response", async () => {
    backend.sendPlayerCommand.mockResolvedValue({
      revision: 12,
      queue: [{ id: "active-track" }],
      playback: { revision: 12, currentItemId: "active-track" },
      settings: {},
      layout: {},
    });

    await player({ type: "load", itemId: "active-track" });

    expect(backend.sendPlayerCommand).toHaveBeenCalledOnce();
    expect(store.acceptSnapshot).not.toHaveBeenCalled();
  });
});
