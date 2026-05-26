use lineage::{Lineage, Node};
use serde::Serialize;
use sqruff_lib::api::{
    Engine, EngineOptions, LintDiagnostic, Mode, ParseErrors, Source, SourceId, SqruffError,
};
use sqruff_lib::config::FluffConfig;
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::parser::{IndentationConfig, Parser};
use std::borrow::Cow;
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
    engine: Engine,
    config: FluffConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Format,
    Cst,
    Lineage,
    Templater,
    Lexer,
}

impl TryFrom<&str> for Tool {
    type Error = &'static str;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "Format" => Ok(Self::Format),
            "Cst" => Ok(Self::Cst),
            "Lineage" => Ok(Self::Lineage),
            "Templater" => Ok(Self::Templater),
            "Lexer" => Ok(Self::Lexer),
            _ => Err("unsupported tool"),
        }
    }
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
    pub fn new(source: &str) -> std::result::Result<Self, JsValue> {
        let config = FluffConfig::try_from_source(source, None)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let engine = Engine::new(
            config.clone(),
            EngineOptions {
                parse_errors: ParseErrors::Include,
            },
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { engine, config })
    }

    #[wasm_bindgen]
    pub fn check(&self, sql: &str, tool: &str) -> Result {
        let Ok(tool) = Tool::try_from(tool) else {
            return Result {
                diagnostics: Vec::new(),
                secondary: format!("Error: unsupported tool: {tool}"),
            };
        };

        match tool {
            Tool::Format => self.check_with_engine(sql, Mode::Fix),
            Tool::Cst | Tool::Lineage | Tool::Templater | Tool::Lexer => {
                self.check_developer_tool(sql, tool)
            }
        }
    }

    fn check_with_engine(&self, sql: &str, mode: Mode) -> Result {
        let report = match self.engine_report(sql, mode) {
            Ok(report) => report,
            Err(e) => return result_from_error(e),
        };

        Result {
            diagnostics: diagnostics_from_lint_diagnostics(&report.diagnostics),
            secondary: report.fixed_source.unwrap_or_default(),
        }
    }

    fn check_developer_tool(&self, sql: &str, tool: Tool) -> Result {
        let report = match self.engine_report(sql, Mode::Check) {
            Ok(report) => report,
            Err(e) => return result_from_error(e),
        };
        let secondary = match tool {
            Tool::Cst => {
                let parsed = match self.engine.parse_source(stdin_source(sql)) {
                    Ok(parsed) => parsed,
                    Err(e) => return result_from_error(e),
                };
                match parsed.tree {
                    Some(cst) => cst.stringify(false),
                    None => parsed
                        .skipped
                        .map(|reason| reason.message)
                        .unwrap_or_default(),
                }
            }
            Tool::Lineage => {
                let parser = Parser::new(self.config.dialect(), IndentationConfig::default());
                let (tables, node) = Lineage::new(parser, "", sql).build();

                print_tree(&tables, node, "", "", "")
            }
            Tool::Templater => {
                let rendered = match self.engine.render_source_for_debug(stdin_source(sql)) {
                    Ok(rendered) => rendered,
                    Err(e) => return result_from_error(e),
                };
                match rendered.templated_file {
                    Some(templated_file) => templated_file.to_yaml(),
                    None => rendered
                        .skipped
                        .map(|reason| reason.message)
                        .unwrap_or_default(),
                }
            }
            Tool::Lexer => {
                let lexed = match self.engine.lex_source_for_debug(stdin_source(sql)) {
                    Ok(lexed) => lexed,
                    Err(e) => return result_from_error(e),
                };
                format_lexer_output(&lexed.segments)
            }
            Tool::Format => String::new(),
        };

        Result {
            diagnostics: diagnostics_from_lint_diagnostics(&report.diagnostics),
            secondary,
        }
    }

    fn engine_report(
        &self,
        sql: &str,
        mode: Mode,
    ) -> std::result::Result<sqruff_lib::api::FileReport, SqruffError> {
        let source = Source {
            id: SourceId::Stdin,
            text: Cow::Borrowed(sql),
        };

        match mode {
            Mode::Check => self.engine.check_source(source),
            Mode::Fix => self.engine.fix_source(source),
        }
    }
}

fn stdin_source(sql: &str) -> Source<'_> {
    Source {
        id: SourceId::Stdin,
        text: Cow::Borrowed(sql),
    }
}

fn diagnostics_from_lint_diagnostics(diagnostics: &[LintDiagnostic]) -> Vec<Diagnostic> {
    diagnostics.iter().map(to_wasm_diagnostic).collect()
}

fn to_wasm_diagnostic(diagnostic: &LintDiagnostic) -> Diagnostic {
    Diagnostic {
        message: diagnostic.message.clone(),
        start_line_number: diagnostic.line as u32,
        start_column: diagnostic.column as u32,
        end_line_number: diagnostic.end_line as u32,
        end_column: diagnostic.end_column as u32,
    }
}

fn result_from_error(error: SqruffError) -> Result {
    result_from_str(&error.to_string())
}

fn result_from_str(message: &str) -> Result {
    Result {
        diagnostics: vec![Diagnostic {
            message: message.to_string(),
            start_line_number: 1,
            start_column: 1,
            end_line_number: 1,
            end_column: 1,
        }],
        secondary: String::new(),
    }
}

fn print_tree(
    tables: &lineage::ir::Tables,
    node: Node,
    parent_prefix: &str,
    immediate_prefix: &str,
    parent_suffix: &str,
) -> String {
    use std::fmt::Write;

    let node_data = &tables.nodes[node];

    let name = &node_data.name;
    let source = tables.stringify(node_data.source);
    let expression = tables.stringify(node_data.expression);
    let source_name = &node_data.source_name;
    let reference_node_name = &&node_data.reference_node_name;

    let mut string = String::new();

    let _ = writeln!(
        string,
        "{:1$}{parent_prefix}{immediate_prefix}name: {name}",
        "", 0
    );
    let _ = writeln!(
        string,
        "{:1$}{parent_prefix}{immediate_prefix}source: {source}",
        "", 0
    );
    let _ = writeln!(
        string,
        "{:1$}{parent_prefix}{immediate_prefix}expression: {expression}",
        "", 0
    );
    let _ = writeln!(
        string,
        "{:1$}{parent_prefix}{immediate_prefix}source_name: {source_name}",
        "", 0
    );
    let _ = writeln!(
        string,
        "{:1$}{parent_prefix}{immediate_prefix}reference_node_name: {reference_node_name}",
        "", 0
    );

    let mut it = node_data.downstream.iter().peekable();
    let child_prefix = format!("{parent_prefix}{parent_suffix}");

    while let Some(child) = it.next().copied() {
        let ret = match it.peek() {
            None => print_tree(tables, child, &child_prefix, "└─ ", "   "),
            Some(_) => print_tree(tables, child, &child_prefix, "├─ ", "│  "),
        };

        string.push_str(&ret);
    }

    string
}

#[derive(Serialize)]
struct LexerOutput {
    tokens: Vec<Token>,
}

#[derive(Serialize)]
struct Token {
    index: usize,
    #[serde(rename = "type")]
    kind: String,
    raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<String>,
}

fn format_lexer_output(segments: &[ErasedSegment]) -> String {
    let tokens: Vec<Token> = segments
        .iter()
        .enumerate()
        .map(|(i, segment)| {
            let position = segment
                .get_position_marker()
                .map(|pos| format!("{}..{}", pos.source_slice.start, pos.source_slice.end));

            Token {
                index: i,
                kind: format!("{:?}", segment.get_type()),
                raw: segment.raw().to_string(),
                position,
            }
        })
        .collect();

    let output = LexerOutput { tokens };
    serde_yaml::to_string(&output).unwrap_or_else(|e| format!("Error serializing: {}", e))
}
