import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installGlobalUiErrorCapture } from "./lib/ui-error-log";
import { applyTheme, readTheme } from "./lib/theme";
import "./styles.css";

installGlobalUiErrorCapture();
applyTheme(readTheme());

if ("__TAURI_INTERNALS__" in window) {
  document.documentElement.dataset.runtime = "tauri";
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
