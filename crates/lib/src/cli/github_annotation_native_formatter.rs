use crate::core::config::FluffConfig;
use crate::core::linter::linted_file::LintedFile;
use std::io::{Stderr, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use super::formatters::Formatter;

#[derive(Debug)]
pub struct GithubAnnotationNativeFormatter {
    output_stream: Stderr,
    pub has_fail: AtomicBool,
}

impl GithubAnnotationNativeFormatter {
    pub fn new(stderr: Stderr) -> Self {
        Self {
            output_stream: stderr,
            has_fail: AtomicBool::new(false),
        }
    }

    fn dispatch(&self, s: &str) {
        let _ignored = self.output_stream.lock().write(s.as_bytes()).unwrap();
    }
}

impl Formatter for GithubAnnotationNativeFormatter {
    fn dispatch_template_header(
        &self,
        _f_name: String,
        _linter_config: FluffConfig,
        _file_config: FluffConfig,
    ) {
        // No-op
    }

    fn dispatch_parse_header(&self, _f_name: String) {
        // No-op
    }

    fn dispatch_file_violations(&self, linted_file: &LintedFile, _only_fixable: bool) {
        let mut violations = linted_file.get_violations(None);

        violations.sort_by(|a, b| {
            a.line_no
                .cmp(&b.line_no)
                .then_with(|| a.line_pos.cmp(&b.line_pos))
                .then_with(|| {
                    let b = b.rule.as_ref().unwrap().code;
                    a.rule.as_ref().unwrap().code.cmp(b)
                })
        });

        for violation in violations {
            let message = format!(
                "::error title=sqruff,file={},line={},col={}::{}: {}\n",
                linted_file.path,
                violation.line_no,
                violation.line_pos,
                violation.rule.as_ref().unwrap().code,
                violation.description
            );
            self.dispatch(&message);
            if !violation.ignore && !violation.warning {
                self.has_fail.store(true, Ordering::SeqCst);
            }
        }
    }

    fn has_fail(&self) -> bool {
        self.has_fail.load(Ordering::SeqCst)
    }

    fn completion_message(&self) {
        // No-op
    }
}
