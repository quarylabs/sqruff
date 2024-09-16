use line_index::LineIndex;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter as SqruffLinter;
use sqruff_lib_core::parser::segments::base::Tables;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone)]
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
#[derive(PartialEq, Eq)]
pub enum Tool {
    Format = "Format",
    Cst = "Cst",
    None = "None",
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Result {
    diagnostics: Vec<Diagnostic>,
    secondary: String,
}

#[wasm_bindgen]
impl Result {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result {
        Result::default()
    }
}

#[wasm_bindgen]
impl Result {
    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn secondary(&self) -> String {
        self.secondary.clone()
    }
}

#[wasm_bindgen]
impl Linter {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Self {
        Self { base: SqruffLinter::new(FluffConfig::from_source(source), None, None) }
    }

    #[wasm_bindgen]
    pub fn check(&self, sql: &str, tool: Tool) -> Result {
        let line_index = LineIndex::new(sql);

        let rule_pack = self.base.get_rulepack().rules();

        let tables = Tables::default();
        let parsed = self.base.parse_string(&tables, sql, None, None).unwrap();

        let mut cst = None;
        if tool == Tool::Cst {
            cst = parsed.tree.clone();
        }

        let mut result = self.base.lint_parsed(&tables, parsed, rule_pack, tool == Tool::Format);
        let violations = &mut result.violations;

        let diagnostics = violations
            .iter_mut()
            .map(|violation| {
                let start = line_index.line_col(violation.source_slice.start.try_into().unwrap());
                let end = line_index.line_col(violation.source_slice.end.try_into().unwrap());

                Diagnostic {
                    message: std::mem::take(&mut violation.description),
                    start_line_number: start.line + 1,
                    start_column: start.col + 1,
                    end_line_number: end.line + 1,
                    end_column: end.col + 1,
                }
            })
            .collect();

        let secondary = match tool {
            Tool::Format => result.fix_string(),
            Tool::Cst => cst.unwrap().stringify(false),
            Tool::None => String::new(),
            Tool::__Invalid => String::new(),
        };

        Result { diagnostics, secondary }
    }
}
