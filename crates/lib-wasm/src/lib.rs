use line_index::LineIndex;
use lineage::{Lineage, Node};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter as SqruffLinter;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_core::parser::{IndentationConfig as ParserIndentationConfig, Parser};
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
    Lineage = "Lineage",
    Templater = "Templater",
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
        let config = FluffConfig::from_source(source, None).unwrap_or_default();
        Self {
            base: SqruffLinter::new(config, None, None, true),
        }
    }

    #[wasm_bindgen]
    pub fn check(&self, sql: &str, tool: Tool) -> Result {
        let line_index = LineIndex::new(sql);

        let tables = Tables::default();
        let parsed = self.base.parse_string(&tables, sql, None).unwrap();

        let templated = self
            .base
            .render_string(sql, "".to_string(), self.base.config())
            .unwrap();

        let cst = if tool == Tool::Cst {
            parsed.tree.clone()
        } else {
            None
        };

        let result = self.base.lint_parsed(&tables, parsed, tool == Tool::Format);
        let violations = result.violations();

        let diagnostics = violations
            .iter()
            .map(|violation| {
                let start = line_index.line_col(violation.source_slice.start.try_into().unwrap());
                let end = line_index.line_col(violation.source_slice.end.try_into().unwrap());

                Diagnostic {
                    message: violation.description.clone(),
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
            Tool::Lineage => {
                let dialect = self
                    .base
                    .config()
                    .dialect()
                    .expect("Dialect is disabled. Please enable the corresponding feature.");
                let indentation = &self.base.config().indentation;
                let indentation_config = ParserIndentationConfig::from_bool_lookup(|key| match key {
                    "indented_joins" => indentation.indented_joins.unwrap_or_default(),
                    "indented_using_on" => indentation.indented_using_on.unwrap_or_default(),
                    "indented_on_contents" => indentation.indented_on_contents.unwrap_or_default(),
                    "indented_then" => indentation.indented_then.unwrap_or_default(),
                    "indented_then_contents" => indentation.indented_then_contents.unwrap_or_default(),
                    "indented_joins_on" => indentation.indented_joins_on.unwrap_or_default(),
                    "indented_ctes" => indentation.indented_ctes.unwrap_or_default(),
                    _ => false,
                });
                let parser = Parser::new(&dialect, indentation_config);
                let (tables, node) = Lineage::new(parser, "", sql).build();

                print_tree(&tables, node, "", "", "")
            }
            Tool::Templater => templated.templated_file.to_yaml(),
            Tool::__Invalid => String::from("Error: unsupported tool"),
        };

        Result {
            diagnostics,
            secondary,
        }
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
