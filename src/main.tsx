import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./i18n";
import "./styles.css";
import App from "./App";
import { startSnapshotBridge } from "./lib/store";

void startSnapshotBridge();

const root = document.getElementById("root");
if (!root) throw new Error("Missing #root mount point");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
