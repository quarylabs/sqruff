mod semantic;

use std::path::{Path, PathBuf};

use hashbrown::HashMap;
use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification, PublishDiagnostics,
};
use lsp_types::request::{Formatting, Request as _, SemanticTokensFullRequest};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    InitializeParams, InitializeResult, NumberOrString, OneOf, Position, PublishDiagnosticsParams,
    Registration, SemanticTokens, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind,
    Uri, VersionedTextDocumentIdentifier,
};
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use sqruff_lib::core::config::ConfigLoader;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
#[cfg(not(target_arch = "wasm32"))]
use sqruff_lib::ignore::IgnoreFile;
use sqruff_lib::templaters::RAW_TEMPLATER;
use wasm_bindgen::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
fn load_config(root: Option<&Path>) -> FluffConfig {
    if let Some(root) = root {
        let loader = ConfigLoader {};
        loader
            .try_load_config_at_path(root)
            .map(|config| FluffConfig::new(config, None, None))
            .unwrap_or_default()
    } else {
        FluffConfig::from_root(None, false, None).unwrap_or_default()
    }
}

#[cfg(target_arch = "wasm32")]
fn load_config(_root: Option<&Path>) -> FluffConfig {
    FluffConfig::default()
}

fn server_initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL).into(),
            document_formatting_provider: OneOf::Left(true).into(),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: semantic::legend(),
                    full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                    range: Some(false),
                    ..Default::default()
                }),
            ),
            ..Default::default()
        },
        server_info: None,
    }
}

pub struct LanguageServer {
    linter: Linter,
    send_diagnostics_callback: Box<dyn Fn(PublishDiagnosticsParams)>,
    documents: HashMap<Uri, String>,
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

        Self(LanguageServer::new(|diagnostics| {
            let diagnostics = serde_wasm_bindgen::to_value(&diagnostics).unwrap();
            send_diagnostics_callback
                .call1(&JsValue::null(), &diagnostics)
                .unwrap();
        }))
    }

    #[wasm_bindgen(js_name = saveRegistrationOptions)]
    pub fn save_registration_options() -> JsValue {
        serde_wasm_bindgen::to_value(&save_registration_options()).unwrap()
    }

    #[wasm_bindgen(js_name = updateConfig)]
    pub fn update_config(&mut self, source: &str) {
        *self.0.linter.config_mut() = FluffConfig::from_source(source, None);
        self.0.recheck_files();
    }

    #[wasm_bindgen(js_name = onInitialize)]
    pub fn on_initialize(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&server_initialize_result()).unwrap()
    }

    #[wasm_bindgen(js_name = onNotification)]
    pub fn on_notification(&mut self, method: &str, params: JsValue) {
        self.0
            .on_notification(method, serde_wasm_bindgen::from_value(params).unwrap())
    }

    #[wasm_bindgen]
    pub fn format(&mut self, uri: JsValue) -> JsValue {
        let uri = serde_wasm_bindgen::from_value(uri).unwrap();
        let edits = self.0.format(uri);
        serde_wasm_bindgen::to_value(&edits).unwrap()
    }

    #[wasm_bindgen(js_name = formatSource)]
    pub fn format_source(&mut self, source: &str) -> String {
        self.0.format_source(source, None)
    }

    #[wasm_bindgen(js_name = semanticTokens)]
    pub fn semantic_tokens(&self, uri: JsValue) -> JsValue {
        let uri = serde_wasm_bindgen::from_value(uri).unwrap();
        let tokens = self.0.semantic_tokens(&uri);
        serde_wasm_bindgen::to_value(&tokens).unwrap()
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
        let config = load_config(Some(&workspace_root));

        #[cfg(target_arch = "wasm32")]
        let _ = workspace_root;

        #[cfg(target_arch = "wasm32")]
        let config = load_config(None);

        let templater = Linter::get_templater(&config).unwrap_or(&RAW_TEMPLATER);
        Self {
            linter: Linter::new(config, None, Some(templater), false).unwrap(),
            send_diagnostics_callback: Box::new(send_diagnostics_callback),
            documents: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            ignore_file: load_ignore_file(&workspace_root),
            #[cfg(not(target_arch = "wasm32"))]
            workspace_root,
        }
    }

    fn on_request(&mut self, id: RequestId, method: &str, params: Value) -> Option<Response> {
        match method {
            Formatting::METHOD => {
                let DocumentFormattingParams {
                    text_document: TextDocumentIdentifier { uri },
                    ..
                } = serde_json::from_value(params).unwrap();

                let edits = self.format(uri);
                Some(Response::new_ok(id, edits))
            }
            SemanticTokensFullRequest::METHOD => {
                let SemanticTokensParams {
                    text_document: TextDocumentIdentifier { uri },
                    ..
                } = serde_json::from_value(params).unwrap();

                let tokens = self.semantic_tokens(&uri);
                Some(Response::new_ok(id, SemanticTokensResult::Tokens(tokens)))
            }
            _ => None,
        }
    }

    fn semantic_tokens(&self, uri: &Uri) -> SemanticTokens {
        if self.is_ignored(uri) {
            return SemanticTokens::default();
        }

        let Some(text) = self.documents.get(uri) else {
            return SemanticTokens::default();
        };

        let filename = file_uri_to_path(uri).map(|path| path.to_string_lossy().to_string());
        let data = semantic::semantic_tokens(&self.linter, text, filename);
        SemanticTokens {
            result_id: None,
            data,
        }
    }

    fn format(&mut self, uri: Uri) -> Vec<lsp_types::TextEdit> {
        if self.is_ignored(&uri) {
            return Vec::new();
        }

        let text = self.documents.get(&uri).cloned().unwrap();
        let filename = file_uri_to_path(&uri).map(|path| path.to_string_lossy().to_string());
        let new_text = self.format_source(&text, filename);
        self.documents.insert(uri.clone(), new_text.clone());
        Self::build_edits(new_text)
    }

    fn format_source(&mut self, source: &str, filename: Option<String>) -> String {
        match self.linter.lint_string(source, filename, true) {
            Ok(tree) => tree.fix_string(),
            Err(e) => {
                eprintln!("Failed to format source: {}", e.value);
                source.to_string()
            }
        }
    }

    fn build_edits(new_text: String) -> Vec<lsp_types::TextEdit> {
        let start_position = Position {
            line: 0,
            character: 0,
        };
        let end_position = Position {
            line: new_text.lines().count() as u32,
            character: new_text.chars().count() as u32,
        };

        vec![lsp_types::TextEdit {
            range: lsp_types::Range::new(start_position, end_position),
            new_text,
        }]
    }

    pub fn on_notification(&mut self, method: &str, params: Value) {
        match method {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(params).unwrap();
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
                let params: DidChangeTextDocumentParams = serde_json::from_value(params).unwrap();

                let content = params.content_changes[0].text.clone();
                let VersionedTextDocumentIdentifier { uri, version: _ } = params.text_document;

                self.check_file(uri.clone(), &content);
                self.documents.insert(uri, content);
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = serde_json::from_value(params).unwrap();
                self.documents.remove(&params.text_document.uri);
            }
            DidSaveTextDocument::METHOD => {
                let params: DidSaveTextDocumentParams = serde_json::from_value(params).unwrap();
                let uri = params.text_document.uri.as_str();

                if uri.ends_with(".sqlfluff") || uri.ends_with(".sqruff") {
                    if self.reload_config() {
                        self.recheck_files();
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

        let filename = file_uri_to_path(&uri).map(|path| path.to_string_lossy().to_string());
        let result = match self.linter.lint_string(text, filename, false) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to check file: {}", e.value);
                return;
            }
        };

        let diagnostics = result
            .into_violations()
            .into_iter()
            .map(|violation| {
                let range = {
                    let pos = Position::new(
                        (violation.line_no as u32).saturating_sub(1),
                        (violation.line_pos as u32).saturating_sub(1),
                    );
                    lsp_types::Range::new(pos, pos)
                };

                let code = violation
                    .rule
                    .map(|rule| NumberOrString::String(rule.code.to_string()));

                Diagnostic::new(
                    range,
                    DiagnosticSeverity::WARNING.into(),
                    code,
                    Some("sqruff".to_string()),
                    violation.description,
                    None,
                    None,
                )
            })
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

    fn reload_config(&mut self) -> bool {
        let new_config = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                load_config(Some(&self.workspace_root))
            }

            #[cfg(target_arch = "wasm32")]
            {
                load_config(None)
            }
        };

        if Linter::get_templater(&new_config).is_ok() {
            *self.linter.config_mut() = new_config;
            true
        } else {
            eprintln!("Invalid templater in config, keeping previous configuration");
            false
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

pub fn run() {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start().unwrap();

    let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    let initialize_result = serde_json::to_value(server_initialize_result()).unwrap();
    connection.initialize_finish(id, initialize_result).unwrap();

    main_loop(connection, init_param);

    io_threads.join().unwrap();
}

fn main_loop(connection: Connection, init_param: InitializeParams) {
    let sender = connection.sender.clone();
    let workspace_root = workspace_root_from_initialize(&init_param);
    let mut lsp = LanguageServer::new_with_workspace_root(workspace_root, move |diagnostics| {
        let notification = new_notification::<PublishDiagnostics>(diagnostics);
        sender.send(Message::Notification(notification)).unwrap();
    });

    let params = save_registration_options();
    connection
        .sender
        .send(Message::Request(Request::new(
            "textDocument-didSave".to_owned().into(),
            "client/registerCapability".to_owned(),
            params,
        )))
        .unwrap();

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("sqruff-lsp-test-{}-{suffix}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn path_to_uri(path: &Path) -> Uri {
        let path = path.canonicalize().unwrap();
        let path = path.to_string_lossy().replace('\\', "/");
        let uri = if cfg!(windows) {
            format!("file:///{path}")
        } else {
            format!("file://{path}")
        };
        uri.parse().unwrap()
    }

    fn diagnostics_lsp(root: &Path) -> (LanguageServer, Arc<Mutex<Vec<PublishDiagnosticsParams>>>) {
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let sent_diagnostics = Arc::clone(&diagnostics);
        let lsp =
            LanguageServer::new_with_workspace_root(Some(root.to_path_buf()), move |params| {
                sent_diagnostics.lock().unwrap().push(params)
            });

        (lsp, diagnostics)
    }

    #[test]
    fn loads_config_from_workspace_root() {
        let root = TempRoot::new();
        fs::write(root.path.join(".sqruff"), "[sqruff]\ndialect = postgres\n").unwrap();

        let (lsp, _) = diagnostics_lsp(&root.path);

        assert_eq!(lsp.linter.config().dialect_kind().as_ref(), "postgres");
    }

    #[test]
    fn clears_diagnostics_for_sqruffignored_file() {
        let root = TempRoot::new();
        fs::write(
            root.path.join(".sqruff"),
            "[sqruff]\ndialect = ansi\nrules = all\n",
        )
        .unwrap();
        fs::write(root.path.join(".sqruffignore"), "ignored.sql\n").unwrap();

        let checked = root.path.join("checked.sql");
        fs::write(&checked, "select  1").unwrap();
        let ignored = root.path.join("ignored.sql");
        fs::write(&ignored, "select  1").unwrap();

        let (lsp, diagnostics) = diagnostics_lsp(&root.path);

        lsp.check_file(path_to_uri(&checked), "select  1");
        assert!(
            !diagnostics
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .diagnostics
                .is_empty(),
            "test fixture should produce diagnostics for a non-ignored file",
        );

        lsp.check_file(path_to_uri(&ignored), "select  1");
        assert!(
            diagnostics
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .diagnostics
                .is_empty(),
            "ignored files should publish an empty diagnostics set",
        );
    }

    #[test]
    fn skips_formatting_sqruffignored_file() {
        let root = TempRoot::new();
        fs::write(
            root.path.join(".sqruff"),
            "[sqruff]\ndialect = ansi\nrules = all\n",
        )
        .unwrap();
        fs::write(root.path.join(".sqruffignore"), "ignored.sql\n").unwrap();

        let ignored = root.path.join("ignored.sql");
        fs::write(&ignored, "select  1").unwrap();

        let (mut lsp, _) = diagnostics_lsp(&root.path);
        let uri = path_to_uri(&ignored);
        lsp.documents.insert(uri.clone(), "select  1".to_string());

        assert!(
            lsp.format(uri).is_empty(),
            "ignored files should not receive formatting edits",
        );
    }

    #[test]
    fn file_uri_with_localhost_authority_is_supported() {
        let uri: Uri = if cfg!(windows) {
            "file://localhost/C:/tmp/sqruff.sql".parse().unwrap()
        } else {
            "file://localhost/tmp/sqruff.sql".parse().unwrap()
        };

        let path = file_uri_to_path(&uri).unwrap();
        assert!(path.ends_with("sqruff.sql"));
    }

    #[test]
    fn file_uri_with_non_local_authority_is_rejected() {
        let uri: Uri = "file://example.com/tmp/sqruff.sql".parse().unwrap();
        assert!(file_uri_to_path(&uri).is_none());
    }
}
