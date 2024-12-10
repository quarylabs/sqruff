use std::sync::Mutex;

use crate::core::{config::FluffConfig, linter::linted_file::LintedFile};

use super::{
    formatters::Formatter,
    json_types::{Diagnostic, DiagnosticCollection, DiagnosticSeverity},
};

#[derive(Default)]
pub struct JsonFormatter {
    violations: Mutex<DiagnosticCollection>,
}

impl Formatter for JsonFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile, only_fixable: bool) {
        let violations = linted_file.get_violations(only_fixable.then_some(true));
        let mut lock = self.violations.lock().unwrap();
        lock.entry(linted_file.path.clone()).or_default().extend(
            violations
                .iter()
                .map(|err| Diagnostic::from(err.clone()))
                .collect::<Vec<_>>(),
        );
    }

    fn has_fail(&self) -> bool {
        let lock = self.violations.lock().unwrap();
        lock.values().any(|v| {
            v.iter()
                .any(|d| matches!(&d.severity, DiagnosticSeverity::Error))
        })
    }

    fn completion_message(&self) {
        let lock = self.violations.lock().unwrap();
        let json = serde_json::to_string(&*lock).unwrap();
        println!("{}", json);
    }

    fn dispatch_template_header(
        &self,
        _f_name: String,
        _linter_config: FluffConfig,
        _file_config: FluffConfig,
    ) {
    }

    fn dispatch_parse_header(&self, _f_name: String) {}
}
