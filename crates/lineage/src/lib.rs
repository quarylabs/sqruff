use std::collections::HashMap;
use std::fmt::Display;

use indexmap::{IndexMap, IndexSet};
use ir::{Expr, ExprKind, Tables};
use schema::Schema;
use scope::{Scope, ScopeKind, Source};
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::segments::ErasedSegment;

mod expand;
pub mod ir;
mod qualify;
mod schema;
mod scope;
mod trie;

pub struct Lineage<'config> {
    parser: Parser<'config>,
    schema: HashMap<String, HashMap<String, String>>,
    column: String,
    segment: ErasedSegment,
    sources: HashMap<String, ErasedSegment>,
    trim_selects: bool,
}

impl<'config> Lineage<'config> {
    pub fn new(parser: Parser<'config>, column: &str, sql: &str) -> Self {
        let parsed = parse_sql(&parser, sql);

        Self {
            segment: parsed,
            parser,
            column: column.to_string(),
            schema: HashMap::new(),
            sources: HashMap::new(),
            trim_selects: true,
        }
    }

    pub fn disable_trim_selects(mut self) -> Self {
        self.trim_selects = false;
        self
    }

    pub fn schema(mut self, name: &str, value: HashMap<String, String>) -> Self {
        self.schema.insert(name.to_string(), value);
        self
    }

    pub fn source(mut self, name: &str, value: &str) -> Self {
        let value = parse_sql(&self.parser, value);
        self.sources.insert(name.to_string(), value);
        self
    }

    pub fn build(self) -> (Tables, Node) {
        let schema = Schema::new(self.schema);
        let (mut tables, expr) = ir::lower(self.segment);

        if !self.sources.is_empty() {
            expand::expand(&mut tables, &self.sources, expr);
        };

        qualify::qualify(&mut tables, &schema, expr);

        let scope = scope::build(&mut tables, expr);
        let node = to_node(&mut tables, &self.column, scope, None, None, None, None);

        (tables, node)
    }
}

fn parse_sql(parser: &Parser, source: &str) -> ErasedSegment {
    let tables = sqruff_lib_core::parser::segments::Tables::default();
    let lexer = parser.dialect().lexer();

    let (tokens, _) = lexer.lex(&tables, source);

    let tables = sqruff_lib_core::parser::segments::Tables::default();
    parser.parse(&tables, &tokens).unwrap().unwrap()
}

pub type Node = usize;

#[derive(Default)]
pub struct NodeData {
    pub name: String,
    pub source: Expr,
    pub expression: Expr,
    pub downstream: Vec<Node>,
    pub source_name: String,
    pub reference_node_name: String,
}

#[derive(Debug, Clone)]
enum Column {
    String(String),
    Index(usize),
}

impl Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Column::String(v) => write!(f, "{v}"),
            Column::Index(v) => write!(f, "{v}"),
        }
    }
}

impl Column {
    fn find_select(&self, tables: &Tables, projections: &[Expr], scope: &Scope) -> Expr {
        match self {
            Column::String(column) => {
                let fallback = || {
                    if tables.is_star(scope.expr()) {
                        tables.alloc_expr(ExprKind::Star, None)
                    } else {
                        scope.expr()
                    }
                };

                projections
                    .iter()
                    .find(|&&it| &tables.alias_or_name(it) == column)
                    .copied()
                    .unwrap_or_else(fallback)
            }
            &Column::Index(index) => projections[index],
        }
    }

    fn to_index(&self, tables: &Tables, projections: &[Expr]) -> Column {
        let idx = match self {
            Column::String(column) => projections
                .iter()
                .position(|&it| &tables.alias_or_name(it) == column || tables.is_star(it))
                .unwrap(),
            &Column::Index(idx) => idx,
        };

        Column::Index(idx)
    }
}

impl From<&str> for Column {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<&String> for Column {
    fn from(value: &String) -> Self {
        Self::String(value.to_string())
    }
}

impl From<usize> for Column {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

fn to_node(
    tables: &mut Tables,
    column: impl Into<Column>,
    scope: Scope,
    scope_name: Option<String>,
    upstream: Option<Node>,
    source_name: Option<String>,
    reference_node_name: Option<String>,
) -> Node {
    let column: Column = column.into();
    let projections = tables.selects(scope.expr());

    let source = scope.expr();
    let select = column.find_select(tables, &projections, &scope);

    if let ExprKind::Subquery(_, _) = &tables.exprs[scope.expr()].kind {
        todo!()
    }

    if let ExprKind::Union { .. } = &tables.exprs[scope.expr()].kind {
        let name = "UNION";
        let upstream = upstream.unwrap_or_else(|| {
            tables.alloc_node(NodeData {
                name: name.to_string(),
                expression: select,
                source: scope.expr(),
                ..Default::default()
            })
        });

        let index = column.to_index(tables, &projections);

        if let Some(union_scopes) = &scope.get().union_scopes {
            for s in union_scopes {
                to_node(
                    tables,
                    index.clone(),
                    s.clone(),
                    None,
                    Some(upstream),
                    source_name.clone(),
                    reference_node_name.clone(),
                );
            }
        }

        return upstream;
    }

    let node = tables.alloc_node(NodeData {
        name: if let Some(scope_name) = scope_name {
            format!("{scope_name}.{column}")
        } else {
            column.to_string()
        },
        source,
        expression: select,
        downstream: Vec::new(),
        source_name: source_name.clone().unwrap_or_default(),
        reference_node_name: reference_node_name.clone().unwrap_or_default(),
    });

    if let Some(upstream) = upstream {
        tables.nodes[upstream].downstream.push(node);
    }

    let subquery_scopes: HashMap<_, _> = scope
        .get()
        .subquery_scopes()
        .iter()
        .map(|subquery_scope| (subquery_scope.expr(), subquery_scope.clone()))
        .collect();

    for expr in scope::walk_in_scope(tables, select) {
        let Some(subquery_scope) = subquery_scopes.get(&expr) else {
            continue;
        };

        let ExprKind::Select { projections, .. } = &tables.exprs[expr].kind else {
            unimplemented!()
        };

        #[allow(clippy::unnecessary_to_owned)] // borrow checker error
        for projection in projections.clone() {
            if let &ExprKind::Alias0(_, name) = &tables.exprs[projection].kind {
                let name = tables.stringify(name);
                to_node(
                    tables,
                    &name,
                    subquery_scope.clone(),
                    None,
                    node.into(),
                    None,
                    None,
                );
            };
        }
    }

    if tables.is_star(select) {
        for source in scope.get().sources.values() {
            let source = match source {
                &Source::Expr(expr) => expr,
                Source::Scope(scope) => scope.expr(),
            };

            let new_node = tables.alloc_node(NodeData {
                name: tables.stringify(select),
                source,
                expression: source,
                ..Default::default()
            });
            tables.nodes[node].downstream.push(new_node);
        }
    }

    // FIXME:
    let source_columns = if tables
        .stringify(select)
        .split('.')
        .collect::<Vec<_>>()
        .len()
        == 1
    {
        IndexSet::new()
    } else {
        scope::walk_in_scope(tables, select)
            .into_iter()
            .filter_map(|it| match &tables.exprs[it].kind {
                ExprKind::Column(column) => Some(column.clone()),
                ExprKind::Alias(column, _) => Some(column.clone()),
                _ => None,
            })
            .collect()
    };

    let derived_tables = &scope.stats(tables).derived_tables;
    let source_names: IndexMap<String, String> = derived_tables
        .iter()
        .filter_map(|&dt_key| -> Option<(String, String)> {
            let dt = tables.exprs[dt_key]
                .comments
                .first()
                .cloned()
                .unwrap_or_default();
            let prefix = dt.strip_prefix("source: ")?;
            let alias = tables.alias(dt_key, false);

            Some((alias, prefix.to_string()))
        })
        .collect();

    for source_column in source_columns {
        let mut iter = source_column.split(".");
        let table = iter.next().unwrap().to_string();
        let column_name = iter
            .next()
            .unwrap()
            .to_string()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();

        let scope_ref = scope.get();
        let Some(source) = scope_ref.sources.get(&table) else {
            continue;
        };

        match source {
            &Source::Expr(source) => {
                let new_node = tables.alloc_node(NodeData {
                    name: source_column,
                    source,
                    expression: source,
                    ..Default::default()
                });

                tables.nodes[node].downstream.push(new_node);
            }
            Source::Scope(source) => {
                let source_name = source_names
                    .get(&table)
                    .cloned()
                    .or_else(|| source_name.clone());
                let mut reference_node_name = None;

                if source.get().kind() == ScopeKind::Cte
                    && let Some((selected_node, _)) = scope.selected_sources(tables).get(&table)
                {
                    let ExprKind::TableReference(name, _) = &tables.exprs[*selected_node].kind
                    else {
                        unimplemented!()
                    };
                    reference_node_name = Some(name.clone());
                }

                to_node(
                    tables,
                    &column_name,
                    source.clone(),
                    table.into(),
                    Some(node),
                    source_name,
                    reference_node_name.clone(),
                );
            }
        }
    }

    node
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::dialects::Dialect;
    use sqruff_lib_core::parser::Parser;
    use strum::IntoEnumIterator;

    use crate::Lineage;

    // Helper function to get all available dialects for testing
    fn all_dialects() -> Vec<(&'static str, Dialect)> {
        DialectKind::iter()
            .filter_map(|kind| {
                let name = kind.name();
                sqruff_lib_dialects::kind_to_dialect(&kind, None).map(|dialect| (name, dialect))
            })
            .collect()
    }

    #[test]
    fn test_lineage() {
        for (dialect_name, dialect) in all_dialects() {
            let parser = Parser::new(&dialect, Default::default());

            let (tables, node) = Lineage::new(parser, "a", "SELECT a FROM z")
                .source("y", "SELECT * FROM x")
                .source("z", "SELECT a FROM y")
                .schema("x", HashMap::from_iter([("a".into(), "int".into())]))
                .build();

            let node_data = &tables.nodes[node];

            assert_eq!(
                &node_data.source_name,
                "",
                "Failed for dialect: {}",
                dialect_name
            );
            assert_eq!(
                tables.stringify(node_data.source),
                "select z.a as a from (select y.a as a from (select x.a as a from x as x) as y) as z",
                "Failed for dialect: {}",
                dialect_name
            );

            let downstream = &tables.nodes[node_data.downstream[0]];
            assert_eq!(
                tables.stringify(downstream.source),
                "select y.a as a from (select x.a as a from x as x) as y",
                "Failed for dialect: {}",
                dialect_name
            );
            assert_eq!(
                &downstream.source_name, "z",
                "Failed for dialect: {}",
                dialect_name
            );

            let downstream = &tables.nodes[downstream.downstream[0]];
            assert_eq!(
                tables.stringify(downstream.source),
                "select x.a as a from x as x",
                "Failed for dialect: {}",
                dialect_name
            );
            assert_eq!(
                &downstream.source_name, "y",
                "Failed for dialect: {}",
                dialect_name
            );
        }
    }

    #[test]
    fn test_lineage_sql_with_cte() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) =
            Lineage::new(parser, "a", "WITH z AS (SELECT a FROM y) SELECT a FROM z")
                .source("y", "SELECT * FROM x")
                .schema("x", HashMap::from_iter([("a".into(), "int".into())]))
                .build();

        let node_data = &tables.nodes[node];

        assert_eq!(
            tables.stringify(node_data.source),
            "with z as (select y.a as a from (select x.a as a from x as x) as y) select z.a as a \
             from z as z"
        );
        assert_eq!(node_data.source_name, "");
        assert_eq!(node_data.reference_node_name, "");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "select y.a as a from (select x.a as a from x as x) as y"
        );
        assert_eq!(downstream.source_name, "");
        assert_eq!(downstream.reference_node_name, "z");

        let downstream = &tables.nodes[downstream.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "select x.a as a from x as x"
        );
        assert_eq!(downstream.source_name, "y");
        assert_eq!(downstream.reference_node_name, "");
    }

    #[test]
    fn test_lineage_source_with_cte() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(parser, "a", "SELECT a FROM z")
            .schema("x", HashMap::from_iter([("a".into(), "int".into())]))
            .source("z", "WITH y AS (SELECT * FROM x) SELECT a FROM y")
            .build();

        let node_data = &tables.nodes[node];
        assert_eq!(
            tables.stringify(node_data.source),
            "select z.a as a from (with y as (select x.a as a from x as x) select y.a as a from y \
             as y) as z"
        );
        assert_eq!(node_data.source_name, "");
        assert_eq!(node_data.reference_node_name, "");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "with y as (select x.a as a from x as x) select y.a as a from y as y"
        );
        assert_eq!(downstream.source_name, "z");
        assert_eq!(downstream.reference_node_name, "");

        let downstream = &tables.nodes[downstream.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "select x.a as a from x as x"
        );
        assert_eq!(downstream.source_name, "z");
        assert_eq!(downstream.reference_node_name, "y");
    }

    #[test]
    fn test_lineage_source_with_star() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) =
            Lineage::new(parser, "a", "WITH y AS (SELECT * FROM x) SELECT a FROM y").build();

        let node_data = &tables.nodes[node];
        assert_eq!(
            tables.stringify(node_data.source),
            "with y as (select * from x as x) select y.a as a from y as y"
        );
        assert_eq!(node_data.source_name, "");
        assert_eq!(node_data.reference_node_name, "");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(tables.stringify(downstream.source), "select * from x as x");
        assert_eq!(downstream.source_name, "");
        assert_eq!(downstream.reference_node_name, "y");
    }

    #[test]
    fn test_lineage_external_col() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser.clone(),
            "a",
            "WITH y AS (SELECT * FROM x) SELECT a FROM y JOIN z USING (uid)",
        )
        .build();

        let node_data = &tables.nodes[node];
        dbg!(tables.stringify(node_data.source));
    }

    #[test]
    fn test_lineage_values() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(parser, "a", "SELECT a FROM y")
            .source("y", "SELECT a FROM (VALUES (1), (2)) AS t (a)")
            .build();

        let node_data = &tables.nodes[node];
        assert_eq!(
            tables.stringify(node_data.source),
            "select y.a as a from (select t.a as a from VALUES (1), (2) t as (a)) as y"
        );
        assert_eq!(node_data.source_name, "");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "select t.a as a from VALUES (1), (2) t as (a)"
        );
        assert_eq!(tables.stringify(downstream.expression), "t.a as a");
        assert_eq!(downstream.source_name, "y");

        let downstream = &tables.nodes[downstream.downstream[0]];
        assert_eq!(
            tables.stringify(downstream.source),
            "VALUES (1), (2) t as (a)"
        );
        assert_eq!(tables.stringify(downstream.expression), "a");
        assert_eq!(downstream.source_name, "y");
    }

    #[test]
    fn test_lineage_cte_name_appears_in_schema() {}

    #[test]
    fn test_lineage_union() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser.clone(),
            "x",
            "SELECT ax AS x FROM a UNION SELECT bx FROM b UNION SELECT cx FROM c",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.downstream.len(), 3);

        let (tables, node) = Lineage::new(
            parser,
            "x",
            "SELECT x FROM (SELECT ax AS x FROM a UNION SELECT bx FROM b UNION SELECT cx FROM c)",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.downstream.len(), 3);
    }

    #[test]
    #[ignore = "TODO"]
    fn test_lineage_lateral_flatten() {
        let dialect = sqruff_lib_dialects::snowflake::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "VALUE",
            "SELECT FLATTENED.VALUE FROM TEST_TABLE, LATERAL FLATTEN(INPUT => RESULT, OUTER => \
             TRUE) FLATTENED",
        )
        .build();
        let node_data = &tables.nodes[node];

        assert_eq!(&node_data.name, "VALUE");

        let downstream = &tables.nodes[node_data.downstream[0]];
        dbg!(&downstream.name);
    }

    #[test]
    fn test_subquery() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser.clone(),
            "output",
            "SELECT (SELECT max(t3.my_column) my_column FROM foo t3) AS output FROM
        table3",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "output");

        let node_data = &tables.nodes[node_data.downstream[0]];
        assert_eq!(node_data.name, "my_column");

        let node_data = &tables.nodes[node_data.downstream[0]];
        assert_eq!(node_data.name, "t3.my_column");
        assert_eq!(tables.stringify(node_data.source), "foo as t3");

        let (tables, node) = Lineage::new(
            parser,
            "y",
            "SELECT SUM((SELECT max(a) a from x) + (SELECT min(b) b from x) + c) AS y FROM x",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "y");
        assert_eq!(tables.nodes[node_data.downstream[0]].name, "a");
        assert_eq!(tables.nodes[node_data.downstream[1]].name, "b");
        assert_eq!(tables.nodes[node_data.downstream[2]].name, "x.c");
    }

    #[test]
    fn test_lineage_cte_union() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "x",
            "WITH dataset AS (
            SELECT *
            FROM catalog.db.table_a

            UNION

            SELECT *
            FROM catalog.db.table_b
        )

        SELECT x, created_at FROM dataset;",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "x");

        let downstream_a = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream_a.name, "0");
        assert_eq!(
            tables.stringify(downstream_a.source),
            "select * from catalog.db.table_a as catalog.db.table_a"
        );
        assert_eq!(downstream_a.reference_node_name, "dataset");

        let downstream_a = &tables.nodes[node_data.downstream[1]];
        assert_eq!(downstream_a.name, "0");
        assert_eq!(
            tables.stringify(downstream_a.source),
            "select * from catalog.db.table_b as catalog.db.table_b"
        );
        assert_eq!(downstream_a.reference_node_name, "dataset");
    }

    #[test]
    fn test_lineage_source_union() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(parser, "x", "SELECT x, created_at FROM dataset;")
            .source(
                "dataset",
                "SELECT *
                FROM catalog.db.table_a

                UNION

                SELECT *
                FROM catalog.db.table_b",
            )
            .build();

        let node_data = &tables.nodes[node];
        assert_eq!(&node_data.name, "x");

        let downstream_a = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream_a.name, "0");
        assert_eq!(downstream_a.source_name, "dataset");
        assert_eq!(
            tables.stringify(downstream_a.source),
            "select * from catalog.db.table_a as catalog.db.table_a"
        );
        assert_eq!(downstream_a.reference_node_name, "");

        let downstream_a = &tables.nodes[node_data.downstream[1]];
        assert_eq!(downstream_a.name, "0");
        assert_eq!(downstream_a.source_name, "dataset");
        assert_eq!(
            tables.stringify(downstream_a.source),
            "select * from catalog.db.table_b as catalog.db.table_b"
        );
        assert_eq!(downstream_a.reference_node_name, "");
    }

    #[test]
    fn test_select_star() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) =
            Lineage::new(parser, "x", "SELECT x from (SELECT * from table_a)").build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "x");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream.name, "_q_0.x");
        assert_eq!(
            tables.stringify(downstream.source),
            "select * from table_a as table_a"
        );

        let downstream = &tables.nodes[downstream.downstream[0]];
        assert_eq!(&downstream.name, "*");
        assert_eq!(tables.stringify(downstream.source), "table_a as table_a");
    }

    #[test]
    fn test_unnest() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "b",
            "with _data as (select [struct(1 as a, 2 as b)] as col) select b from _data cross \
             join unnest(col)",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "b");
    }

    #[test]
    #[ignore = "TODO:"]
    fn test_lineage_normalize() {
        let dialect = sqruff_lib_dialects::snowflake::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser.clone(),
            "a",
            "WITH x AS (SELECT 1 a) SELECT a FROM x",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "A");

        let (_tables, _node) =
            Lineage::new(parser, "\"a\"", "WITH x AS (SELECT 1 a) SELECT a FROM x").build();
    }

    #[test]
    #[ignore = "The Oracle dialect is not supported."]
    fn test_ddl_lineage() {}

    #[test]
    fn test_trim() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "a",
            "SELECT a, b, c\nFROM (select a, b, c from y) z",
        )
        .disable_trim_selects()
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "a");
        assert_eq!(
            tables.stringify(node_data.source),
            "select z.a as a, z.b as b, z.c as c from (select y.a as a, y.b as b, y.c as c from y \
             as y) as z"
        );

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream.name, "z.a");
        assert_eq!(
            tables.stringify(downstream.source),
            "select y.a as a, y.b as b, y.c as c from y as y"
        );
    }

    #[test]
    fn test_node_name_doesnt_contain_comment() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "x",
            "SELECT * FROM (SELECT x /* c */ FROM t1) AS t2",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.downstream.len(), 1);
        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream.downstream.len(), 1);
        assert_eq!(tables.nodes[downstream.downstream[0]].name, "t1.x");
    }

    #[test]
    fn test_lineage_downstream_id_in_join() {
        let dialect = sqruff_lib_dialects::ansi::dialect(None);
        let parser = Parser::new(&dialect, Default::default());

        let (tables, node) = Lineage::new(
            parser,
            "id",
            "SELECT u.name, t.id FROM users AS u INNER JOIN tests AS t ON u.id = t.id",
        )
        .build();

        let node_data = &tables.nodes[node];
        assert_eq!(node_data.name, "id");

        let downstream = &tables.nodes[node_data.downstream[0]];
        assert_eq!(downstream.name, "t.id");
    }
}
