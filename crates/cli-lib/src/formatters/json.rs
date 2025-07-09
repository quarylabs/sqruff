use std::sync::Mutex;

use sqruff_lib::{Formatter, core::linter::linted_file::LintedFile};

use super::json_types::{Diagnostic, DiagnosticCollection};

#[derive(Default)]
pub(crate) struct JsonFormatter {
    violations: Mutex<DiagnosticCollection>,
}

impl Formatter for JsonFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile) {
        let violations = linted_file.violations();
        let mut lock = self.violations.lock().unwrap();
        lock.entry(linted_file.path().into()).or_default().extend(
            violations
                .iter()
                .map(|err| Diagnostic::from(err.clone()))
                .collect::<Vec<_>>(),
        );
    }

    fn completion_message(&self, _count: usize) {
        let lock = self.violations.lock().unwrap();
        let json = serde_json::to_string(&*lock).unwrap();
        println!("{json}");
    }
}
