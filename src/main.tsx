import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { AppRouterProvider } from "@/app/providers/router-provider";

import "./index.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <AppRouterProvider />
  </StrictMode>,
);
