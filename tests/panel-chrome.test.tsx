import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PanelChrome } from "../src/components/PanelChrome";

const { startWindowDragging } = vi.hoisted(() => ({
  startWindowDragging: vi.fn(async () => undefined),
}));

vi.mock("../src/lib/backend", () => ({ startWindowDragging }));

describe("PanelChrome", () => {
  afterEach(() => vi.clearAllMocks());

  it("starts native dragging from the title without installing a native drag overlay", () => {
    const { container } = render(<PanelChrome title="Tonelag">content</PanelChrome>);

    fireEvent.mouseDown(screen.getByText("Tonelag"), { button: 0 });

    expect(startWindowDragging).toHaveBeenCalledOnce();
    expect(container.querySelector("[data-tauri-drag-region]")).toBeNull();
  });

  it("lets title-bar controls receive mouse clicks", () => {
    const clicked = vi.fn();
    render(
      <PanelChrome title="Tonelag" controls={<button onClick={clicked}>Options</button>} expandedDragArea>
        content
      </PanelChrome>,
    );

    fireEvent.mouseDown(screen.getByRole("button", { name: "Options" }), { button: 0 });
    fireEvent.click(screen.getByRole("button", { name: "Options" }));

    expect(startWindowDragging).not.toHaveBeenCalled();
    expect(clicked).toHaveBeenCalledOnce();
  });

  it("starts dragging from unused space in a full-width title control layer", () => {
    const { container } = render(
      <PanelChrome className="main-panel" title="Tonelag" controls={<button>Options</button>} expandedDragArea>
        content
      </PanelChrome>,
    );

    fireEvent.mouseDown(container.querySelector(".panel-controls")!, { button: 0 });

    expect(startWindowDragging).toHaveBeenCalledOnce();
  });
});
