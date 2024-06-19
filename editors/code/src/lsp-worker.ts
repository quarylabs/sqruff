import sqruffInit, * as sqruffLsp from "../dist/lsp";
import sqruffWasmData from "../dist/lsp_bg.wasm";
import * as vscode from "vscode";

import {
  createConnection,
  BrowserMessageReader,
  BrowserMessageWriter,
  PublishDiagnosticsParams,
  RequestType,
  DocumentFormattingParams,
} from "vscode-languageserver/browser";

sqruffInit(sqruffWasmData).then(() => {
  const reader = new BrowserMessageReader(self);
  const writer = new BrowserMessageWriter(self);

  const connection = createConnection(reader, writer);

  async function loadConfig(): Promise<string> {
    return await connection.sendRequest("loadConfig");
  }

  const sendDiagnosticsCallback = (params: PublishDiagnosticsParams) =>
    connection.sendDiagnostics(params);

  let lsp = new sqruffLsp.Wasm(sendDiagnosticsCallback);

  connection.onInitialize(() => lsp.onInitialize());
  connection.onRequest(
    "textDocument/formatting",
    (params: DocumentFormattingParams) => {
      return lsp.format(params.textDocument.uri);
    },
  );
  connection.onNotification((...args) => {
    loadConfig().then((config) => {

    });
    lsp.onNotification(...args);
  });
  connection.listen();

  self.postMessage("OK");
});
