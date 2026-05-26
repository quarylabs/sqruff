use hashbrown::HashMap;
use ignore::gitignore::Gitignore;
use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification, PublishDiagnostics,
};
use lsp_types::request::{Formatting, Request as _};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    InitializeParams, InitializeResult, NumberOrString, OneOf, Position, PublishDiagnosticsParams,
    Registration, ServerCapabilities, TextDocumentItem,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri, VersionedTextDocumentIdentifier,
};
use serde_json::Value;
use sqruff_lib::api::{
    Engine, EngineOptions, LintDiagnostic, ParseErrors, Source, SourceId, SqruffError,
};
use sqruff_lib::core::config::FluffConfig;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
fn load_config() -> FluffConfig {
    FluffConfig::from_root(None, false, None).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn load_config() -> FluffConfig {
    FluffConfig::default()
}

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
    engine: Engine,
    send_diagnostics_callback: Box<dyn Fn(PublishDiagnosticsParams)>,
    documents: HashMap<Uri, String>,
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
            match serde_wasm_bindgen::to_value(&diagnostics) {
                Ok(diagnostics) => {
                    if let Err(e) =
                        send_diagnostics_callback.call1(&JsValue::null(), &diagnostics)
                    {
                        eprintln!("Failed to send diagnostics: {e:?}");
                    }
                }
                Err(e) => eprintln!("Failed to serialize diagnostics: {e:?}"),
            }
        }))
    }

    #[wasm_bindgen(js_name = saveRegistrationOptions)]
    pub fn save_registration_options() -> JsValue {
        serde_wasm_bindgen::to_value(&save_registration_options()).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(js_name = updateConfig)]
    pub fn update_config(&mut self, source: &str) {
        let new_config = match FluffConfig::try_from_source(source, None) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("Invalid config, keeping previous configuration: {error}");
                return;
            }
        };
        if self.0.set_config(new_config).is_ok() {
            self.0.recheck_files();
        } else {
            eprintln!("Invalid templater in config, keeping previous configuration");
        }
    }

    #[wasm_bindgen(js_name = onInitialize)]
    pub fn on_initialize(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&server_initialize_result()).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(js_name = onNotification)]
    pub fn on_notification(&mut self, method: &str, params: JsValue) {
        match serde_wasm_bindgen::from_value(params) {
            Ok(params) => self.0.on_notification(method, params),
            Err(e) => eprintln!("Failed to deserialize notification params: {e:?}"),
        }
    }

    #[wasm_bindgen]
    pub fn format(&mut self, uri: JsValue) -> JsValue {
        let uri = match serde_wasm_bindgen::from_value(uri) {
            Ok(uri) => uri,
            Err(e) => {
                eprintln!("Failed to deserialize uri: {e:?}");
                return JsValue::NULL;
            }
        };
        let edits = self.0.format(uri);
        serde_wasm_bindgen::to_value(&edits).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(js_name = formatSource)]
    pub fn format_source(&mut self, source: &str) -> String {
        self.0.format_source(source)
    }
}

impl LanguageServer {
    pub fn new(send_diagnostics_callback: impl Fn(PublishDiagnosticsParams) + 'static) -> Self {
        let config = load_config();
        let engine = Self::new_engine(config).unwrap_or_else(|e| {
            eprintln!("Failed to create engine from config, using defaults: {e}");
            Self::new_engine(FluffConfig::default())
                .expect("default config must produce a valid engine")
        });
        Self {
            engine,
            send_diagnostics_callback: Box::new(send_diagnostics_callback),
            documents: HashMap::new(),
        }
    }

    fn on_request(&mut self, id: RequestId, method: &str, params: Value) -> Option<Response> {
        match method {
            Formatting::METHOD => {
                let params: DocumentFormattingParams = match serde_json::from_value(params) {
                    Ok(p) => p,
                    Err(e) => {
                        return Some(Response::new_err(
                            id,
                            lsp_server::ErrorCode::InvalidParams as i32,
                            e.to_string(),
                        ));
                    }
                };
                let edits = self.format(params.text_document.uri);
                Some(Response::new_ok(id, edits))
            }
            _ => None,
        }
    }

    fn format(&mut self, uri: Uri) -> Vec<lsp_types::TextEdit> {
        let text = match self.documents.get(&uri).cloned() {
            Some(text) => text,
            None => return Vec::new(),
        };
        let new_text = self.format_source(&text);
        build_full_document_edit(&text, new_text)
    }

    fn format_source(&mut self, source: &str) -> String {
        match self.engine.fix_source(Source {
            id: SourceId::Stdin,
            text: Cow::Borrowed(source),
        }) {
            Ok(report) => report.fixed_source.unwrap_or_else(|| source.to_string()),
            Err(e) => {
                eprintln!("Failed to format source: {e}");
                source.to_string()
            }
        }
    }

    fn set_config(&mut self, new_config: FluffConfig) -> Result<(), SqruffError> {
        self.engine.reload_config(new_config)?;
        Ok(())
    }

    fn new_engine(config: FluffConfig) -> Result<Engine, SqruffError> {
        Engine::new(
            config,
            EngineOptions {
                parse_errors: ParseErrors::Include,
            },
        )
    }

    pub fn on_notification(&mut self, method: &str, params: Value) {
        match method {
            DidOpenTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(params)
                else {
                    return;
                };
                let TextDocumentItem {
                    uri,
                    language_id: _,
                    version: _,
                    text,
                } = params.text_document;

                self.check_file(uri.clone(), &text);
                self.documents.insert(uri, text);
            }
            DidChangeTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidChangeTextDocumentParams>(params)
                else {
                    return;
                };

                let content = params.content_changes[0].text.clone();
                let VersionedTextDocumentIdentifier { uri, version: _ } = params.text_document;

                self.check_file(uri.clone(), &content);
                self.documents.insert(uri, content);
            }
            DidCloseTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidCloseTextDocumentParams>(params)
                else {
                    return;
                };
                self.documents.remove(&params.text_document.uri);
            }
            DidSaveTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidSaveTextDocumentParams>(params)
                else {
                    return;
                };
                let uri = params.text_document.uri.as_str();

                if uri.ends_with(".sqlfluff") || uri.ends_with(".sqruff") {
                    let new_config = load_config();
                    if self.set_config(new_config).is_ok() {
                        self.recheck_files();
                    } else {
                        eprintln!("Invalid templater in config, keeping previous configuration");
                    }
                }
            }
            _ => {}
        }
    }

    fn recheck_files(&mut self) {
        for (uri, text) in self.documents.iter() {
            self.check_file(uri.clone(), text);
        }
    }

    fn check_file(&self, uri: Uri, text: &str) {
        if Self::is_ignored(&uri) {
            let diagnostics = PublishDiagnosticsParams::new(uri.clone(), Vec::new(), None);
            (self.send_diagnostics_callback)(diagnostics);
            return;
        }

        let report = match self.engine.check_source(Source {
            id: source_id_from_uri(&uri),
            text: Cow::Borrowed(text),
        }) {
            Ok(report) => report,
            Err(e) => {
                eprintln!("Failed to check file: {e}");
                return;
            }
        };

        let diagnostics = report
            .diagnostics
            .iter()
            .map(|diag| to_lsp_diagnostic(diag, text))
            .collect();

        let diagnostics = PublishDiagnosticsParams::new(uri.clone(), diagnostics, None);
        (self.send_diagnostics_callback)(diagnostics);
    }

    fn is_ignored(uri: &Uri) -> bool {
        let Some(path) = Self::uri_to_file_path(uri) else {
            return false;
        };
        let Ok(root) = std::env::current_dir() else {
            return false;
        };
        let ignore_file = root.join(".sqruffignore");
        if !ignore_file.exists() {
            return false;
        }

        let (gitignore, err) = Gitignore::new(ignore_file);
        if err.is_some() {
            return false;
        }

        gitignore.matched(&path, path.is_dir()).is_ignore()
    }

    fn uri_to_file_path(uri: &Uri) -> Option<PathBuf> {
        if uri.scheme()?.as_str() != "file" {
            return None;
        }

        let mut path = uri.path().as_str().to_string();
        #[cfg(windows)]
        {
            if path.len() >= 3 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' {
                path.remove(0);
            }
        }

        Some(Path::new(&path).to_path_buf())
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let init_param: InitializeParams = serde_json::from_value(params)?;
    let initialize_result = serde_json::to_value(server_initialize_result())?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(connection, init_param);

    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: Connection, _init_param: InitializeParams) {
    let sender = connection.sender.clone();
    let mut lsp = LanguageServer::new(move |diagnostics| {
        let notification = new_notification::<PublishDiagnostics>(diagnostics);
        if let Err(e) = sender.send(Message::Notification(notification)) {
            eprintln!("Failed to send diagnostics notification: {e}");
        }
    });

    let params = save_registration_options();
    connection
        .sender
        .send(Message::Request(Request::new(
            "textDocument-didSave".to_owned().into(),
            "client/registerCapability".to_owned(),
            params,
        )))
        .unwrap_or_else(|e| eprintln!("Failed to send registration request: {e}"));

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).unwrap_or(false) {
                    return;
                }

                if let Some(response) = lsp.on_request(request.id, &request.method, request.params)
                {
                    if let Err(e) = connection.sender.send(Message::Response(response)) {
                        eprintln!("Failed to send response: {e}");
                    }
                }
            }
            Message::Response(_) => {}
            Message::Notification(notification) => {
                lsp.on_notification(&notification.method, notification.params);
            }
        }
    }
}

pub fn save_registration_options() -> lsp_types::RegistrationParams {
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

    lsp_types::RegistrationParams {
        registrations: vec![Registration {
            id: "textDocument/didSave".into(),
            method: "textDocument/didSave".into(),
            register_options: serde_json::to_value(save_registration_options)
                .unwrap()
                .into(),
        }],
    }
}

fn new_notification<T>(params: T::Params) -> lsp_server::Notification
where
    T: Notification,
{
    lsp_server::Notification {
        method: T::METHOD.to_owned(),
        params: serde_json::to_value(&params).unwrap(),
    }
}

fn to_lsp_diagnostic(diag: &LintDiagnostic, source: &str) -> Diagnostic {
    let line_index = LineIndex::new(source);
    let start = line_index.position(diag.source_range.start);
    let end = line_index.position(diag.source_range.end);
    let range = lsp_types::Range::new(start, end);

    let code = diag.code.clone().map(NumberOrString::String);

    Diagnostic::new(
        range,
        DiagnosticSeverity::WARNING.into(),
        code,
        Some("sqruff".to_string()),
        diag.message.clone(),
        None,
        None,
    )
}

fn source_id_from_uri(uri: &Uri) -> SourceId {
    LanguageServer::uri_to_file_path(uri)
        .map_or_else(|| SourceId::Virtual(uri.to_string()), SourceId::Path)
}

fn build_full_document_edit(old_text: &str, new_text: String) -> Vec<lsp_types::TextEdit> {
    vec![lsp_types::TextEdit {
        range: full_document_range(old_text),
        new_text,
    }]
}

fn full_document_range(text: &str) -> lsp_types::Range {
    lsp_types::Range::new(Position::new(0, 0), LineIndex::new(text).end_position())
}

struct LineIndex {
    line_starts: Vec<usize>,
    text: String,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(idx, byte)| (byte == b'\n').then_some(idx + 1)),
        );

        Self {
            line_starts,
            text: text.to_string(),
        }
    }

    fn position(&self, byte_offset: usize) -> Position {
        let byte_offset = byte_offset.min(self.text.len());
        let line = self.line_for_offset(byte_offset);
        let line_start = self.line_starts[line];
        let character = self.text[line_start..byte_offset]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();

        Position::new(line as u32, character as u32)
    }

    fn end_position(&self) -> Position {
        self.position(self.text.len())
    }

    fn line_for_offset(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use lsp_types::notification::{
        DidChangeTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    };
    use lsp_types::{
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        TextDocumentContentChangeEvent, TextDocumentIdentifier,
    };

    use super::*;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    struct Workspace {
        root: PathBuf,
        previous: PathBuf,
    }

    impl Workspace {
        fn new(name: &str, config: &str) -> Self {
            let previous = std::env::current_dir().unwrap();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("sqruff-lsp-{name}-{nanos}"));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join(".sqruff"), config).unwrap();
            std::env::set_current_dir(&root).unwrap();

            Self { root, previous }
        }

        fn uri(&self, relative: &str) -> Uri {
            file_uri(&self.root.join(relative))
        }

        fn write_config(&self, config: &str) {
            fs::write(self.root.join(".sqruff"), config).unwrap();
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.previous).unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn config(dialect: &str, templater: &str) -> String {
        format!("[sqruff]\ndialect = {dialect}\ntemplater = {templater}\n")
    }

    fn file_uri(path: &Path) -> Uri {
        let path = path.to_string_lossy().replace('\\', "/");
        let path = if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        };
        Uri::from_str(&format!("file://{path}")).unwrap()
    }

    fn server_with_diagnostics() -> (LanguageServer, Arc<Mutex<Vec<PublishDiagnosticsParams>>>) {
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&diagnostics);
        let server = LanguageServer::new(move |params| {
            captured.lock().unwrap().push(params);
        });

        (server, diagnostics)
    }

    fn open(server: &mut LanguageServer, uri: Uri, text: &str) {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "sql".to_string(),
                version: 1,
                text: text.to_string(),
            },
        };
        server.on_notification(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(params).unwrap(),
        );
    }

    fn change(server: &mut LanguageServer, uri: Uri, text: &str) {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        };
        server.on_notification(
            DidChangeTextDocument::METHOD,
            serde_json::to_value(params).unwrap(),
        );
    }

    fn save_config(server: &mut LanguageServer, uri: Uri) {
        let params = DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: None,
        };
        server.on_notification(
            DidSaveTextDocument::METHOD,
            serde_json::to_value(params).unwrap(),
        );
    }

    #[test]
    fn invalid_sql_publishes_diagnostics() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("invalid-sql", &config("ansi", "raw"));
        let (mut server, diagnostics) = server_with_diagnostics();

        open(&mut server, workspace.uri("bad.sql"), "SELECT FROM\n");

        let diagnostics = diagnostics.lock().unwrap();
        assert!(!diagnostics.last().unwrap().diagnostics.is_empty());
    }

    #[test]
    fn ignored_files_publish_empty_diagnostics() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("ignored-file", &config("ansi", "raw"));
        fs::write(workspace.root.join(".sqruffignore"), "ignored.sql\n").unwrap();
        let (mut server, diagnostics) = server_with_diagnostics();

        open(&mut server, workspace.uri("ignored.sql"), "SELECT FROM\n");

        let diagnostics = diagnostics.lock().unwrap();
        assert!(diagnostics.last().unwrap().diagnostics.is_empty());
    }

    #[test]
    fn formatting_returns_full_document_edit_for_old_range() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("format-range", &config("ansi", "raw"));
        let (mut server, _diagnostics) = server_with_diagnostics();
        let uri = workspace.uri("format.sql");

        open(&mut server, uri.clone(), "SELECT  1");
        let edits = server.format(uri);

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range.start, Position::new(0, 0));
        assert_eq!(edits[0].range.end, Position::new(0, 9));
        assert_eq!(edits[0].new_text, "SELECT 1\n");
    }

    #[test]
    fn formatting_does_not_mutate_document_before_did_change() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("format-no-mutate", &config("ansi", "raw"));
        let (mut server, _diagnostics) = server_with_diagnostics();
        let uri = workspace.uri("format.sql");

        open(&mut server, uri.clone(), "SELECT  1");
        let edits = server.format(uri.clone());

        assert_eq!(edits[0].new_text, "SELECT 1\n");
        assert_eq!(server.documents.get(&uri).unwrap(), "SELECT  1");

        change(&mut server, uri.clone(), &edits[0].new_text);
        assert_eq!(server.documents.get(&uri).unwrap(), "SELECT 1\n");
    }

    #[test]
    fn changing_dialect_reloads_diagnostics() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("reload-dialect", &config("ansi", "raw"));
        let (mut server, diagnostics) = server_with_diagnostics();
        let uri = workspace.uri("postgres.sql");
        let sql = "SELECT DISTINCT ON (customer_id)\n    customer_id\nFROM orders\nORDER BY customer_id;\n";

        open(&mut server, uri, sql);
        let ansi_count = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        workspace.write_config(&config("postgres", "raw"));
        save_config(&mut server, workspace.uri(".sqruff"));
        let postgres_count = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        assert!(ansi_count > postgres_count);
    }

    #[test]
    fn changing_templater_reloads_templater() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new("reload-templater", &config("postgres", "raw"));
        let (mut server, diagnostics) = server_with_diagnostics();
        let uri = workspace.uri("placeholder.sql");
        let sql = "SELECT :x AS value\n";

        open(&mut server, uri, sql);
        let raw_count = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        workspace.write_config(
            "[sqruff]\ndialect = postgres\ntemplater = placeholder\n\n[sqruff:templater:placeholder]\nparam_style = colon\nx = 1\n",
        );
        save_config(&mut server, workspace.uri(".sqruff"));
        let placeholder_count = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        assert!(raw_count > placeholder_count);
    }

    #[test]
    fn invalid_templater_keeps_previous_config() {
        let _guard = CWD_LOCK.lock().unwrap();
        let workspace = Workspace::new(
            "invalid-templater",
            "[sqruff]\ndialect = postgres\ntemplater = placeholder\n\n[sqruff:templater:placeholder]\nparam_style = colon\nx = 1\n",
        );
        let (mut server, diagnostics) = server_with_diagnostics();
        let uri = workspace.uri("placeholder.sql");
        let sql = "SELECT :x AS value\n";

        open(&mut server, uri, sql);
        let before = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        workspace.write_config("[sqruff]\ndialect = postgres\ntemplater = not_real\n");
        save_config(&mut server, workspace.uri(".sqruff"));
        let after = diagnostics
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .diagnostics
            .len();

        assert_eq!(before, 0);
        assert_eq!(after, 0);
    }
}
