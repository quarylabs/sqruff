use line_index::LineIndex;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter as SqruffLinter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Diagnostic {
    message: String,
    pub start_line_number: u32,
    pub start_column: u32,
    pub end_line_number: u32,
    pub end_column: u32,
}

#[wasm_bindgen]
impl Diagnostic {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

#[wasm_bindgen]
pub struct Linter {
    base: SqruffLinter,
}

#[wasm_bindgen]
impl Linter {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Self {
        Self { base: SqruffLinter::new(FluffConfig::from_source(source), None, None) }
    }

    #[wasm_bindgen]
    pub fn check(&self, sql: &str) -> Vec<Diagnostic> {
        let line_index = LineIndex::new(sql);

        let rule_pack = self.base.get_rulepack().rules();
        let result = self.base.lint_string(sql, None, None, None, rule_pack, false);
        let violations = result.violations;

        violations
            .into_iter()
            .map(|violation| {
                let start = line_index.line_col(violation.source_slice.start.try_into().unwrap());
                let end = line_index.line_col(violation.source_slice.end.try_into().unwrap());

                Diagnostic {
                    message: violation.description,
                    start_line_number: start.line + 1,
                    start_column: start.col + 1,
                    end_line_number: end.line + 1,
                    end_column: end.col + 1,
                }
            })
            .collect()
    }

    #[wasm_bindgen]
    pub fn format(&self, sql: &str) -> String {
        let rule_pack = self.base.get_rulepack().rules();
        let tree = self.base.lint_string(sql, None, None, None, rule_pack, true);
        tree.fix_string()
    }
}
