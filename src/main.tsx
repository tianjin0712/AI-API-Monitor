import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import "./styles/miuix.css";
import "./styles/miuix-official.css";

const cachedTheme = window.localStorage.getItem("ai-monitor-theme");
if (cachedTheme === "dark" || cachedTheme === "light") {
  document.documentElement.dataset.theme = cachedTheme;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
