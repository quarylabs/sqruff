import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/browser";

export function activate(context: vscode.ExtensionContext) {
  const serverMain = vscode.Uri.joinPath(
    context.extensionUri,
    "dist/browserServerMain.js",
  );

  const worker = new Worker(serverMain.toString(true));
  worker.onmessage = (message) => {
    if (message.data !== "OK") {
      return;
    }

    const cl = new LanguageClient(
      "sqruff-lsp",
      "Sqruff LSP",
      { documentSelector: [{ language: "sql" }] },
      worker,
    );

    cl.onRequest("loadFile", async (path: string) => {
      let contents = await vscode.workspace.fs.readFile(
        vscode.Uri.parse(path, true),
      );
      return new TextDecoder().decode(contents);
    });

    cl.start().then(() => {});
  };
}

export function deactivate() {}
