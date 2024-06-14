import sqruffInit, * as sqruff_lsp from "../dist/language_server";
import sqruffWasmData from "../dist/language_server_bg.wasm";

import {
    createConnection,
    BrowserMessageReader,
    BrowserMessageWriter,
    TextDocumentSyncKind,
    PublishDiagnosticsParams
} from "vscode-languageserver/browser";

sqruffInit(sqruffWasmData).then(() => {
    
    const reader = new BrowserMessageReader(self);
    const writer = new BrowserMessageWriter(self);
    
    const connection = createConnection(reader, writer);
    
    const sendDiagnosticsCallback = (params: PublishDiagnosticsParams) =>
        connection.sendDiagnostics(params);
    
    let lsp = new sqruff_lsp.LanguageServer(sendDiagnosticsCallback);

    connection.onInitialize(() => {
        return {
            capabilities: {
                textDocumentSync: {
                    change: TextDocumentSyncKind.Full,
                }
            }
        }
    });

    connection.onNotification((...args) => lsp.onNotification(...args));
    connection.listen();

    self.postMessage("OK");
});

