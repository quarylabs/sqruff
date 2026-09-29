import React from "react";
import ReactDOM from "react-dom/client";
import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import "./index.css";
import Chrome from "./Editor/Chrome";

loader.config({ monaco });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Chrome />
  </React.StrictMode>,
);
