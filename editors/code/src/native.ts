import * as vscode from "vscode";
import * as path from "path";
import * as os from "os";
import { existsSync } from "fs";

import { LanguageClient, ServerOptions } from "vscode-languageclient/node";

const program_extension = process.platform === "win32" ? ".exe" : "";

function resolveExecutablePath(
  inputPath: string | undefined,
  workspaceFolderPath: string | undefined,
): string | undefined {
  if (inputPath === undefined) {
    return undefined;
  }

  const trimmed = inputPath.trim();
  if (trimmed.length === 0) {
    return undefined;
  }

  let resolved = trimmed;
  if (workspaceFolderPath !== undefined) {
    resolved = resolved.replace(/\$\{workspaceFolder\}/g, workspaceFolderPath);
  }

  if (resolved === "~") {
    resolved = os.homedir();
  } else if (
    resolved.startsWith("~/") ||
    resolved.startsWith("~\\") ||
    resolved.startsWith("~" + path.sep)
  ) {
    resolved = path.join(os.homedir(), resolved.slice(2));
  }

  if (
    workspaceFolderPath !== undefined &&
    !path.isAbsolute(resolved) &&
    (resolved.startsWith(".") ||
      resolved.includes("/") ||
      resolved.includes("\\"))
  ) {
    resolved = path.join(workspaceFolderPath, resolved);
  }

  return resolved;
}

function looksLikePath(value: string): boolean {
  return value.startsWith(".") || value.includes("/") || value.includes("\\");
}

export function activate(context: vscode.ExtensionContext) {
  const configuredPath = resolveExecutablePath(
    vscode.workspace.getConfiguration("sqruff").get<string>("executablePath"),
    vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
  );

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

  let serverModule = configuredPath;
  if (serverModule !== undefined && looksLikePath(serverModule)) {
    if (!existsSync(serverModule)) {
      vscode.window.showWarningMessage(
        `sqruff.executablePath is set to "${serverModule}", but the file does not exist. ` +
          "Falling back to bundled sqruff or PATH.",
      );
      serverModule = undefined;
    }
  }

  if (serverModule === undefined) {
    serverModule = lspSearchPaths.find((path) => existsSync(path));
  }
  if (serverModule === undefined) {
    serverModule = "sqruff";
  }

  const args = ["lsp"];
  const serverOptions: ServerOptions = {
    run: { command: serverModule, options: {}, args: args },
    debug: { command: serverModule, options: {}, args: args },
  };

  const cl = new LanguageClient("sqruff-lsp", "Sqruff LSP", serverOptions, {
    documentSelector: [{ language: "sql" }],
  });

  cl.start();
}

export function deactivate() {}
