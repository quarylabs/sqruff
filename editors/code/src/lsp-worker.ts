import sqruffInit, * as sqruffLsp from "../dist/lsp";
import sqruffWasmData from "../dist/lsp_bg.wasm";

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
  connection.onNotification((...args) => lsp.onNotification(...args));
  connection.listen();

  self.postMessage("OK");
});
