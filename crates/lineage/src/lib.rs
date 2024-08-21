mod expand;
mod qualify;
mod schema;
mod scope;

use std::collections::HashMap;

use schema::Schema;
use scope::Scope;
use sqruff_lib::core::linter::linter::Linter;
use sqruff_lib::core::parser::parser::Parser;
use sqruff_lib::core::parser::segments::base::{ErasedSegment, Tables};
use sqruff_lib::core::templaters::base::TemplatedFile;
use sqruff_lib::dialects::SyntaxKind;

pub fn parse_sql(tables: &Tables, parser: &Parser, source: &str) -> ErasedSegment {
    let (tokens, _) = Linter::lex_templated_file(
        tables,
        TemplatedFile::from_string(source.into()),
        parser.config(),
    );

    let tokens = tokens.unwrap_or_default();
    parser.parse(&tables, &tokens, None, false).unwrap().unwrap()
}

struct Lineage<'config> {
    tables: Tables,
    parser: Parser<'config>,
    column: String,
    segment: ErasedSegment,
    sources: HashMap<String, ErasedSegment>,
}

impl<'config> Lineage<'config> {
    fn new(parser: Parser<'config>, column: &str, sql: &str) -> Self {
        let tables = Tables::default();
        let parsed = parse_sql(&tables, &parser, sql);
        let mut stmts = specific_statement_segment(parsed);
        let root = stmts.pop().unwrap();

        Self { segment: root, tables, parser, column: column.to_string(), sources: HashMap::new() }
    }

    fn source(mut self, name: &str, value: &str) -> Self {
        let value = parse_sql(&self.tables, &self.parser, value);
        let mut value = specific_statement_segment(value);
        let value = value.pop().unwrap();
        self.sources.insert(name.to_string(), value);
        self
    }

    fn build(self) {
        let segment = if !self.sources.is_empty() {
            expand::expand(self.segment, &self.sources)
        } else {
            self.segment
        };

        let root = qualify::qualify(segment, Schema::default());
        let scope = scope::build_scope(root);

        to_node(&self.column, scope);
    }
}

fn to_node(column: &str, scope: Scope) {
    println!("{}", scope.segment().raw());
    // println!("{}",
    // serde_yaml::to_string(&scope.segment().to_serialised(false,
    // true)).unwrap());
}

fn specific_statement_segment(parsed: ErasedSegment) -> Vec<ErasedSegment> {
    let mut segments = Vec::new();

    for top_segment in parsed.segments() {
        match top_segment.get_type() {
            SyntaxKind::Statement => {
                segments.push(top_segment.segments()[0].clone());
            }
            SyntaxKind::Batch => unimplemented!(),
            _ => {}
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use sqruff_lib::core::config::FluffConfig;
    use sqruff_lib::core::parser::parser::Parser;

    use crate::Lineage;

    #[test]
    fn test_lineage() {
        let config = FluffConfig::default();
        let parser = Parser::new(&config, None);

        let lineage = Lineage::new(parser, "a", "SELECT a FROM z")
            .source("y", "SELECT * FROM x")
            .source("z", "SELECT a FROM y")
            .build();
    }
}
