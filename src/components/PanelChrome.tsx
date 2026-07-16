import type { MouseEvent, PropsWithChildren, ReactNode } from "react";
import { startWindowDragging } from "../lib/backend";

interface PanelChromeProps extends PropsWithChildren {
  className?: string;
  title: string;
  controls?: ReactNode;
}

export function PanelChrome({ className = "", title, controls, children }: PanelChromeProps) {
  const startDragging = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button, input, select, textarea, a, [role='menuitem']")) return;
    event.preventDefault();
    void startWindowDragging();
  };

  return (
    <section className={`classic-panel ${className}`} aria-label={title}>
      <header className="panel-titlebar" onMouseDown={startDragging}>
        <span className="panel-title">{title}</span>
      </header>
      {controls && <span className="panel-controls">{controls}</span>}
      {children}
    </section>
  );
}
