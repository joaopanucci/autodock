import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./i18n";
import { initTheme } from "./theme";
import "@fontsource-variable/inter/wght.css";
import "./theme-tokens.css";
import "./style.css";

initTheme();

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
