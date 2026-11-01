import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-500.css";
import "@fontsource/jetbrains-mono/latin-600.css";
import App from "./App";
import { Overlay } from "./Overlay";
import "./index.css";

// The same bundle serves the main window and the cheat-sheet overlay.
function isOverlayWindow(): boolean {
  try {
    return getCurrentWindow().label === "overlay";
  } catch {
    return false;
  }
}

const overlay = isOverlayWindow();
if (overlay) {
  document.documentElement.style.background = "transparent";
  document.body.className = "bg-transparent text-neutral-100 antialiased";
}

createRoot(document.getElementById("root")!).render(<StrictMode>{overlay ? <Overlay /> : <App />}</StrictMode>);
