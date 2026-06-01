use hashbrown::HashMap;
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
    Registration, ServerCapabilities, TextDocumentItem, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri, VersionedTextDocumentIdentifier,
};
use serde_json::Value;
use sqruff_lib::api::{
    Engine, EngineOptions, LintDiagnostic, ParseErrors, Source, SourceId, SqruffError,
};
#[cfg(not(target_arch = "wasm32"))]
use sqruff_lib::core::config::ConfigLoader;
use sqruff_lib::core::config::FluffConfig;
#[cfg(not(target_arch = "wasm32"))]
use sqruff_lib::ignore::IgnoreFile;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
fn load_config(root: Option<&Path>) -> Result<FluffConfig, SqruffError> {
    if let Some(root) = root {
        let loader = ConfigLoader {};
        let config = loader.try_load_config_up_to_path(root, None, false)?;
        Ok(FluffConfig::new(config, None, None))
    } else {
        FluffConfig::from_root(None, false, None)
    }
}

#[cfg(target_arch = "wasm32")]
fn load_config(_root: Option<&Path>) -> Result<FluffConfig, SqruffError> {
    Ok(FluffConfig::default())
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
    startup_config_error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    workspace_root: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    ignore_file: IgnoreFile,
}

#[wasm_bindgen]
pub struct Wasm(LanguageServer);

#[wasm_bindgen]
impl Wasm {
    #[wasm_bindgen(constructor)]
    pub fn new(send_diagnostics_callback: js_sys::Function) -> Self {
        console_error_panic_hook::set_once();

        let send_diagnostics_callback = Box::leak(Box::new(send_diagnostics_callback));

        Self(LanguageServer::new(
            |diagnostics| match serde_wasm_bindgen::to_value(&diagnostics) {
                Ok(diagnostics) => {
                    if let Err(e) = send_diagnostics_callback.call1(&JsValue::null(), &diagnostics)
                    {
                        eprintln!("Failed to send diagnostics: {e:?}");
                    }
                }
                Err(e) => eprintln!("Failed to serialize diagnostics: {e:?}"),
            },
        ))
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
        Self::new_with_workspace_root(None, send_diagnostics_callback)
    }

    fn new_with_workspace_root(
        workspace_root: Option<PathBuf>,
        send_diagnostics_callback: impl Fn(PublishDiagnosticsParams) + 'static,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let workspace_root = workspace_root
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        #[cfg(not(target_arch = "wasm32"))]
        let config_root = Some(workspace_root.as_path());

        #[cfg(target_arch = "wasm32")]
        let config_root = None;

        let (config, mut startup_config_error) = match load_config(config_root) {
            Ok(config) => (config, None),
            Err(error) => {
                let message = format!("Failed to load config, using defaults: {error}");
                eprintln!("{message}");
                (FluffConfig::default(), Some(message))
            }
        };
        let engine = match Self::new_engine(config) {
            Ok(engine) => engine,
            Err(error) => {
                let message =
                    format!("Failed to create engine from config, using defaults: {error}");
                eprintln!("{message}");
                if startup_config_error.is_none() {
                    startup_config_error = Some(message);
                }
                Self::new_engine(FluffConfig::default())
                    .expect("default config must produce a valid engine")
            }
        };
        Self {
            engine,
            send_diagnostics_callback: Box::new(send_diagnostics_callback),
            documents: HashMap::new(),
            startup_config_error,
            #[cfg(not(target_arch = "wasm32"))]
            ignore_file: load_ignore_file(&workspace_root),
            #[cfg(not(target_arch = "wasm32"))]
            workspace_root,
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
        if self.is_ignored(&uri) {
            return Vec::new();
        }

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

    pub fn startup_config_error(&self) -> Option<&str> {
        self.startup_config_error.as_deref()
    }

    pub fn on_notification(&mut self, method: &str, params: Value) {
        match method {
            DidOpenTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(params) else {
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
                let Ok(params) = serde_json::from_value::<DidChangeTextDocumentParams>(params)
                else {
                    return;
                };

                let content = params.content_changes[0].text.clone();
                let VersionedTextDocumentIdentifier { uri, version: _ } = params.text_document;

                self.check_file(uri.clone(), &content);
                self.documents.insert(uri, content);
            }
            DidCloseTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value::<DidCloseTextDocumentParams>(params)
                else {
                    return;
                };
                self.documents.remove(&params.text_document.uri);
            }
            DidSaveTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value::<DidSaveTextDocumentParams>(params) else {
                    return;
                };
                let uri = params.text_document.uri.as_str();

                if uri.ends_with(".sqlfluff") || uri.ends_with(".sqruff") {
                    let new_config = match self.load_workspace_config() {
                        Ok(config) => config,
                        Err(error) => {
                            eprintln!("Invalid config, keeping previous configuration: {error}");
                            return;
                        }
                    };
                    if self.set_config(new_config).is_ok() {
                        self.recheck_files();
                    } else {
                        eprintln!("Invalid templater in config, keeping previous configuration");
                    }
                } else if uri.ends_with(".sqruffignore") {
                    self.reload_ignore_file();
                    self.recheck_files();
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
        if self.is_ignored(&uri) {
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

    fn is_ignored(&self, uri: &Uri) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            file_uri_to_path(uri).is_some_and(|path| self.ignore_file.is_ignored(&path))
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = uri;
            false
        }
    }

    fn load_workspace_config(&self) -> Result<FluffConfig, SqruffError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            load_config(Some(&self.workspace_root))
        }

        #[cfg(target_arch = "wasm32")]
        {
            load_config(None)
        }
    }

    fn reload_ignore_file(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.ignore_file = load_ignore_file(&self.workspace_root);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_ignore_file(root: &Path) -> IgnoreFile {
    IgnoreFile::new_from_root(root).unwrap_or_else(|err| {
        eprintln!("Failed to load .sqruffignore: {err}");
        IgnoreFile::empty()
    })
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

fn main_loop(connection: Connection, init_param: InitializeParams) {
    let sender = connection.sender.clone();
    let workspace_root = workspace_root_from_initialize(&init_param);
    let mut lsp = LanguageServer::new_with_workspace_root(workspace_root, move |diagnostics| {
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

#[cfg(not(target_arch = "wasm32"))]
fn workspace_root_from_initialize(params: &InitializeParams) -> Option<PathBuf> {
    params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .and_then(|folder| file_uri_to_path(&folder.uri))
        .or_else(|| {
            #[allow(deprecated)]
            params.root_uri.as_ref().and_then(file_uri_to_path)
        })
        .or_else(|| {
            #[allow(deprecated)]
            params.root_path.as_ref().map(PathBuf::from)
        })
}

#[cfg(target_arch = "wasm32")]
fn workspace_root_from_initialize(_params: &InitializeParams) -> Option<PathBuf> {
    None
}

fn file_uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if uri
        .scheme()
        .is_some_and(|scheme| scheme.eq_lowercase("file"))
    {
        if let Some(authority) = uri.authority() {
            let host = authority.host().as_str();
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return None;
            }
        }

        let path = uri.path().as_estr().decode().into_bytes();

        #[cfg(target_os = "windows")]
        {
            let path = String::from_utf8(path.into_owned()).ok()?;
            let path = if path.starts_with('/')
                && path
                    .as_bytes()
                    .get(2)
                    .is_some_and(|character| *character == b':')
            {
                &path[1..]
            } else {
                path.as_str()
            };
            return Some(PathBuf::from(path.replace('/', "\\")));
        }

        #[cfg(target_family = "unix")]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;

            return Some(PathBuf::from(OsStr::from_bytes(path.as_ref())));
        }

        #[cfg(not(any(target_os = "windows", target_family = "unix")))]
        {
            let path = String::from_utf8(path.into_owned()).ok()?;
            return Some(PathBuf::from(path));
        }
    }

    let uri = uri.as_str();
    let path = uri.strip_prefix("file://")?;
    let path = percent_decode(path)?;

    #[cfg(target_os = "windows")]
    {
        let path = if path.starts_with('/')
            && path
                .as_bytes()
                .get(2)
                .is_some_and(|character| *character == b':')
        {
            &path[1..]
        } else {
            path.as_str()
        };

        return Some(PathBuf::from(path.replace('/', "\\")));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Some(PathBuf::from(path))
    }
}

fn percent_decode(input: &str) -> Option<String> {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.as_bytes().iter().copied();

    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next()?;
            let low = bytes.next()?;
            output.push((hex_value(high)? << 4) | hex_value(low)?);
        } else {
            output.push(byte);
        }
    }

    String::from_utf8(output).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
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
                lsp_types::DocumentFilter {
                    language: None,
                    scheme: None,
                    pattern: Some("**/.sqruffignore".into()),
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
    file_uri_to_path(uri).map_or_else(|| SourceId::Virtual(uri.to_string()), SourceId::Path)
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
    fn startup_config_error_is_visible() {
        let _guard = CWD_LOCK.lock().unwrap();
        let _workspace = Workspace::new(
            "invalid-startup-config",
            "[sqruff]\ntemplater = dbt\n\n[sqruff:templater:dbt]\nproject_dir = 1\n",
        );
        let (server, _diagnostics) = server_with_diagnostics();

        assert!(
            server
                .startup_config_error()
                .is_some_and(|error| error.contains("invalid path value"))
        );
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
