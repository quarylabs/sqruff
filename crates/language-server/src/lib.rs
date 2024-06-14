use lsp_types::notification::{DidChangeTextDocument, Notification};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, PublishDiagnosticsParams,
    VersionedTextDocumentIdentifier,
};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
}

pub(crate) fn log(s: &str) {
    #[allow(unused_unsafe)]
    unsafe {
        console_log(&("[pls] ".to_owned() + s))
    }
}

#[wasm_bindgen]
pub struct LanguageServer {
    send_diagnostics_callback: js_sys::Function,
    linter: Linter,
}

#[wasm_bindgen]
impl LanguageServer {
    #[wasm_bindgen(constructor)]
    pub fn new(send_diagnostics_callback: &js_sys::Function) -> Self {
        console_error_panic_hook::set_once();

        Self {
            send_diagnostics_callback: send_diagnostics_callback.clone(),
            linter: Linter::new(FluffConfig::default(), None, None),
        }
    }

    #[wasm_bindgen(js_name = onNotification)]
    pub fn on_notification(&mut self, method: &str, params: JsValue) {
        log(method);
        log(&format!("{params:?}"));

        match method {
            DidChangeTextDocument::METHOD => {
                log("HELLO");

                let params: DidChangeTextDocumentParams =
                    serde_wasm_bindgen::from_value(params).unwrap();

                let content = params.content_changes[0].text.clone();
                let VersionedTextDocumentIdentifier { uri, version } = params.text_document;

                let rule_pack = self.linter.get_rulepack().rules();
                let result = self.linter.lint_string(
                    content.into(),
                    None,
                    false.into(),
                    None,
                    None,
                    rule_pack,
                    false,
                );

                let diagnostics = result
                    .violations
                    .into_iter()
                    .map(|violation| {
                        Diagnostic::new(
                            to_range((violation.line_no, violation.line_pos)),
                            DiagnosticSeverity::WARNING.into(),
                            None,
                            None,
                            violation.description,
                            None,
                            None,
                        )
                    })
                    .collect();

                self.send_diagnostics(PublishDiagnosticsParams::new(
                    uri,
                    diagnostics,
                    version.into(),
                ));
            }
            _ => {}
        }
    }
}

impl LanguageServer {
    fn send_diagnostics(&self, diagnostics: PublishDiagnosticsParams) {
        let this = &JsValue::null();

        let diagnostics = &serde_wasm_bindgen::to_value(&diagnostics).unwrap();
        if let Err(e) = self.send_diagnostics_callback.call1(this, diagnostics) {
            log(&format!("send_diagnostics params:\n\t{:?}\n\tJS error: {:?}", diagnostics, e));
        }
    }
}

fn to_range(span: (usize, usize)) -> lsp_types::Range {
    let pos = lsp_types::Position::new(
        (span.0 as u32).saturating_sub(1),
        (span.1 as u32).saturating_sub(1),
    );
    lsp_types::Range::new(pos, pos)
}
