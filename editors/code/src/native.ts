import * as vscode from "vscode";
import * as path from "path";
import { existsSync } from "fs";

import {
  LanguageClient,
  ServerOptions,
  ExecutableOptions,
  State,
} from "vscode-languageclient/node";

const program_extension = process.platform === "win32" ? ".exe" : "";

export function activate(context: vscode.ExtensionContext) {
  const lspSearchPaths = [
    path.join(
      context.extensionPath,
      "..",
      "..",
      "target",
      "debug",
      "sqruff" + program_extension,
    ),
    path.join(
      context.extensionPath,
      "..",
      "..",
      "target",
      "release",
      "sqruff" + program_extension,
    ),
  ];

  let serverModule = lspSearchPaths.find((path) => existsSync(path));
  if (serverModule === undefined) {
    serverModule = "sqruff";
  }

  let args = ["lsp"];
  let serverOptions: ServerOptions = {
    run: { command: serverModule, options: {}, args: args },
    debug: { command: serverModule, options: {}, args: args },
  };

  const cl = new LanguageClient("sqruff-lsp", "Sqruff LSP", serverOptions, {
    documentSelector: [{ language: "sql" }],
  });

  cl.start();
}

export function deactivate() { }
