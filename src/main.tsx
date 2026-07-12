import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./app";
import Settings from "./settings";

const isSettings = (() => {
  // Allow browser dev via ?view=settings; prefer the Tauri window label.
  if (new URLSearchParams(window.location.search).get("view") === "settings") return true;
  try {
    return getCurrentWindow().label === "settings";
  } catch {
    return false;
  }
})();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isSettings ? <Settings /> : <App />}</React.StrictMode>,
);
