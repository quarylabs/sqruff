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
      {
        documentSelector: [{ language: "sql" }],
      },
      worker,
    );

    const fileEvents = [
      vscode.workspace.createFileSystemWatcher("**/.sqruff"),
      vscode.workspace.createFileSystemWatcher("**/.sqlfluff"),
    ];

    for (const fileEvent of fileEvents) {
      fileEvent.onDidChange(() => {
        cl.sendRequest("changeConfig");
      });
      fileEvent.onDidCreate(() => {
        cl.sendRequest("changeConfig");
      });
      fileEvent.onDidDelete(() => {
        cl.sendRequest("deleteConfig");
      });
    }

    cl.onRequest("loadConfig", async () => {
      if (vscode.workspace.workspaceFolders === undefined) {
        return "";
      }

      const uri = vscode.workspace.workspaceFolders[0].uri;
      const fileNames = [".sqlfluff", ".sqruff"];
      let contents = new Uint8Array();

      for (const fileName of fileNames) {
        try {
          contents = await vscode.workspace.fs.readFile(
            vscode.Uri.joinPath(uri, fileName),
          );
          break;
        } catch (error) {
          // Continue to the next file if an error occurs
          console.error(error);
        }
      }

      return new TextDecoder().decode(contents);
    });

    cl.start().then(() => {});
  };
}

export function deactivate() {}
