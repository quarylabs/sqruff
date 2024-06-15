use lsp_server::{Connection, Message};
use lsp_types::notification::{
    DidChangeTextDocument, DidOpenTextDocument, Notification, PublishDiagnostics,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    VersionedTextDocumentIdentifier,
};
use serde_json::Value;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;
use wasm_bindgen::prelude::*;

fn server_initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL).into(),
            ..Default::default()
        },
        server_info: None,
    }
}

pub struct LanguageServer {
    linter: Linter,
    send_diagnostics_callback: Box<dyn Fn(PublishDiagnosticsParams)>,
}

#[wasm_bindgen]
pub struct Wasm(LanguageServer);

#[wasm_bindgen]
impl Wasm {
    #[wasm_bindgen(constructor)]
    pub fn new(send_diagnostics_callback: &js_sys::Function) -> Self {
        console_error_panic_hook::set_once();

        let send_diagnostics_callback = Box::leak(Box::new(send_diagnostics_callback.clone()));

        Self(LanguageServer::new(|diagnostics| {
            let diagnostics = serde_wasm_bindgen::to_value(&diagnostics).unwrap();
            send_diagnostics_callback.call1(&JsValue::null(), &diagnostics).unwrap();
        }))
    }

    #[wasm_bindgen(js_name = onInitialize)]
    pub fn on_initialize(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&server_initialize_result()).unwrap()
    }

    #[wasm_bindgen(js_name = onNotification)]
    pub fn on_notification(&mut self, method: &str, params: JsValue) {
        self.0.on_notification(method, serde_wasm_bindgen::from_value(params).unwrap())
    }
}

impl LanguageServer {
    pub fn new(send_diagnostics_callback: impl Fn(PublishDiagnosticsParams) + 'static) -> Self {
        Self {
            linter: Linter::new(FluffConfig::default(), None, None),
            send_diagnostics_callback: Box::new(send_diagnostics_callback),
        }
    }

    pub fn on_notification(&mut self, method: &str, params: Value) {
        match method {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(params).unwrap();
                let TextDocumentItem { uri, language_id: _, version, text } = params.text_document;

                self.check_file(uri, text, version);
            }
            DidChangeTextDocument::METHOD => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(params).unwrap();

                let content = params.content_changes[0].text.clone();
                let VersionedTextDocumentIdentifier { uri, version } = params.text_document;

                self.check_file(uri, content, version);
            }
            _ => {}
        }
    }

    fn check_file(&mut self, uri: Uri, text: String, version: i32) {
        let rule_pack = self.linter.get_rulepack().rules();
        let result =
            self.linter.lint_string(text.into(), None, false.into(), None, None, rule_pack, false);

        let diagnostics = result
            .violations
            .into_iter()
            .map(|violation| {
                let range = {
                    let pos = lsp_types::Position::new(
                        (violation.line_no as u32).saturating_sub(1),
                        (violation.line_pos as u32).saturating_sub(1),
                    );
                    lsp_types::Range::new(pos, pos)
                };

                Diagnostic::new(
                    range,
                    DiagnosticSeverity::WARNING.into(),
                    None,
                    None,
                    violation.description,
                    None,
                    None,
                )
            })
            .collect();

        let diagnostics = PublishDiagnosticsParams::new(uri, diagnostics, version.into());
        (self.send_diagnostics_callback)(diagnostics);
    }
}

pub fn run() {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start().unwrap();

    let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    let initialize_result = serde_json::to_value(server_initialize_result()).unwrap();
    connection.initialize_finish(id, initialize_result).unwrap();

    main_loop(connection, init_param);

    io_threads.join().unwrap();
}
fn main_loop(connection: Connection, _init_param: InitializeParams) {
    let sender = connection.sender.clone();
    let mut lsp = LanguageServer::new(move |diagnostics| {
        let notification = new_notification::<PublishDiagnostics>(diagnostics);
        sender.send(Message::Notification(notification)).unwrap();
    });

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).unwrap() {
                    return;
                }
            }
            Message::Response(_) => {}
            Message::Notification(notification) => {
                lsp.on_notification(&notification.method, notification.params)
            }
        }
    }
}

fn new_notification<T>(params: T::Params) -> lsp_server::Notification
where
    T: lsp_types::notification::Notification,
{
    lsp_server::Notification {
        method: T::METHOD.to_owned(),
        params: serde_json::to_value(&params).unwrap(),
    }
}
