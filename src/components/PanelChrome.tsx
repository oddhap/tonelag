import type { PropsWithChildren, ReactNode } from "react";

interface PanelChromeProps extends PropsWithChildren {
  className?: string;
  title: string;
  controls?: ReactNode;
}

export function PanelChrome({ className = "", title, controls, children }: PanelChromeProps) {
  return (
    <section className={`classic-panel ${className}`} aria-label={title}>
      <header className="panel-titlebar" data-tauri-drag-region>
        <span className="panel-title" data-tauri-drag-region>{title}</span>
        {controls && <span className="panel-controls">{controls}</span>}
      </header>
      {children}
    </section>
  );
}
