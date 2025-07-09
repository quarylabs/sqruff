use std::io::{Stderr, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use sqruff_lib::Formatter;
use sqruff_lib::core::linter::linted_file::LintedFile;

#[derive(Debug)]
pub(crate) struct GithubAnnotationNativeFormatter {
    output_stream: Stderr,
    pub has_fail: AtomicBool,
}

impl GithubAnnotationNativeFormatter {
    pub(crate) fn new(stderr: Stderr) -> Self {
        Self {
            output_stream: stderr,
            has_fail: AtomicBool::new(false),
        }
    }

    fn dispatch(&self, s: &str) {
        let mut output_stream = self.output_stream.lock();
        output_stream
            .write_all(s.as_bytes())
            .and_then(|_| output_stream.flush())
            .unwrap_or_else(|e| panic!("failed to emit error: {e}"));
    }
}

impl Formatter for GithubAnnotationNativeFormatter {
    fn dispatch_file_violations(&self, linted_file: &LintedFile) {
        for violation in linted_file.violations() {
            let message = format!(
                "::error title=sqruff,file={},line={},col={}::{}: {}\n",
                linted_file.path(),
                violation.line_no,
                violation.line_pos,
                violation.rule_code(),
                violation.description
            );

            self.dispatch(&message);
            self.has_fail.store(true, Ordering::SeqCst);
        }
    }

    fn completion_message(&self, _count: usize) {
        // No-op
    }
}
