import type { MouseEvent, PropsWithChildren, ReactNode } from "react";
import { startWindowDragging } from "../lib/backend";

interface PanelChromeProps extends PropsWithChildren {
  className?: string;
  title: string;
  controls?: ReactNode;
  expandedDragArea?: boolean;
}

const interactiveSelector = "button, input, select, textarea, a, label, [role='menu'], [role='menuitem'], [role='listbox'], [role='option'], [contenteditable='true']";

export function PanelChrome({ className = "", title, controls, expandedDragArea = false, children }: PanelChromeProps) {
  const startDragging = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest(interactiveSelector)) return;
    event.preventDefault();
    void startWindowDragging();
  };

  return (
    <section className={`classic-panel ${className}`} aria-label={title} onMouseDown={expandedDragArea ? startDragging : undefined}>
      <header className="panel-titlebar" onMouseDown={expandedDragArea ? undefined : startDragging}>
        <span className="panel-title">{title}</span>
      </header>
      {controls && <span className="panel-controls">{controls}</span>}
      {children}
    </section>
  );
}
