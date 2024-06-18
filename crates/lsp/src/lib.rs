use ahash::AHashMap;
use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification, PublishDiagnostics,
};
use lsp_types::request::{Formatting, Request as _};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    InitializeParams, InitializeResult, OneOf, Position, PublishDiagnosticsParams, Registration,
    ServerCapabilities, TextDocumentIdentifier, TextDocumentItem, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri, VersionedTextDocumentIdentifier,
};
use serde_json::Value;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;
use wasm_bindgen::prelude::*;

fn server_initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL).into(),
            document_formatting_provider: OneOf::Left(true).into(),
            ..Default::default()
        },
        server_info: None,
    }
}

pub struct LanguageServer {
    linter: Linter,
    send_diagnostics_callback: Box<dyn Fn(PublishDiagnosticsParams)>,
    documents: AHashMap<Uri, String>,
}

#[wasm_bindgen]
pub struct Wasm(LanguageServer);

#[wasm_bindgen]
impl Wasm {
    #[wasm_bindgen(constructor)]
    pub fn new(send_diagnostics_callback: js_sys::Function) -> Self {
        console_error_panic_hook::set_once();

        let send_diagnostics_callback = Box::leak(Box::new(send_diagnostics_callback));

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

    #[wasm_bindgen]
    pub fn format(&mut self, uri: JsValue) -> JsValue {
        let uri = serde_wasm_bindgen::from_value(uri).unwrap();
        let edits = self.0.format(uri);
        serde_wasm_bindgen::to_value(&edits).unwrap()
    }
}

impl LanguageServer {
    pub fn new(send_diagnostics_callback: impl Fn(PublishDiagnosticsParams) + 'static) -> Self {
        Self {
            linter: Linter::new(FluffConfig::from_root(None, false, None).unwrap(), None, None),
            send_diagnostics_callback: Box::new(send_diagnostics_callback),
            documents: AHashMap::new(),
        }
    }

    fn on_request(&mut self, id: RequestId, method: &str, params: Value) -> Option<Response> {
        match method {
            Formatting::METHOD => {
                let DocumentFormattingParams {
                    text_document: TextDocumentIdentifier { uri }, ..
                } = serde_json::from_value(params).unwrap();

                let edits = self.format(uri);
                Some(lsp_server::Response::new_ok(id, edits))
            }
            _ => None,
        }
    }

    fn format(&mut self, uri: Uri) -> Vec<lsp_types::TextEdit> {
        let rule_pack = self.linter.get_rulepack().rules();
        let text = &self.documents[&uri];
        let tree = self.linter.lint_string(text, None, None, None, rule_pack, true);

        let new_text = tree.fix_string();
        let start_position = Position { line: 0, character: 0 };
        let end_position = Position {
            line: new_text.lines().count() as u32,
            character: new_text.chars().count() as u32,
        };

        let result = vec![lsp_types::TextEdit {
            range: lsp_types::Range::new(start_position, end_position),
            new_text,
        }];
        result
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
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = serde_json::from_value(params).unwrap();
                self.documents.remove(&params.text_document.uri);
            }
            DidSaveTextDocument::METHOD => {
                let params: DidSaveTextDocumentParams = serde_json::from_value(params).unwrap();
                let uri = params.text_document.uri.as_str();

                if uri.ends_with(".sqlfluff") || uri.ends_with(".sqruff") {
                    *self.linter.config_mut() = FluffConfig::from_root(None, false, None).unwrap();
                }
            }
            _ => {}
        }
    }

    fn check_file(&mut self, uri: Uri, text: String, version: i32) {
        let rule_pack = self.linter.get_rulepack().rules();
        let result = self.linter.lint_string(&text, None, None, None, rule_pack, false);

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

        let diagnostics = PublishDiagnosticsParams::new(uri.clone(), diagnostics, version.into());
        (self.send_diagnostics_callback)(diagnostics);

        self.documents.insert(uri, text);
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

    let save_registration_options = lsp_types::TextDocumentSaveRegistrationOptions {
        include_text: false.into(),
        text_document_registration_options: lsp_types::TextDocumentRegistrationOptions {
            document_selector: Some(vec![
                lsp_types::DocumentFilter {
                    language: None,
                    scheme: None,
                    pattern: Some("**/.sqlfluff".into()),
                },
                lsp_types::DocumentFilter {
                    language: None,
                    scheme: None,
                    pattern: Some("**/.sqruff".into()),
                },
            ]),
        },
    };

    let request = Request::new(
        "textDocument-didSave".to_owned().into(),
        "client/registerCapability".to_owned(),
        lsp_types::RegistrationParams {
            registrations: vec![Registration {
                id: "textDocument/didSave".into(),
                method: "textDocument/didSave".into(),
                register_options: serde_json::to_value(save_registration_options).unwrap().into(),
            }],
        },
    );

    connection.sender.send(Message::Request(request)).unwrap();

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).unwrap() {
                    return;
                }

                if let Some(response) = lsp.on_request(request.id, &request.method, request.params)
                {
                    connection.sender.send(Message::Response(response)).unwrap();
                }
            }
            Message::Response(_) => {}
            Message::Notification(notification) => {
                lsp.on_notification(&notification.method, notification.params);
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
