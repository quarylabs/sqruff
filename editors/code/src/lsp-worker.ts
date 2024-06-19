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

  async function loadFile(path: string): Promise<string> {
    return await connection.sendRequest("loadFile", path);
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
    console.log(args);
    loadFile("vscode-test-web://mount/alter_sequence.sql").then((val) => {
      console.log(val);
    });
    lsp.onNotification(...args);
  });
  connection.listen();

  self.postMessage("OK");
});
