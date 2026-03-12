use expect_test::expect_file;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::value::Value;
use sqruff_lib_dialects::postgres;

use hashbrown::HashMap;

fn check_no_unparsable_segments(tree: &ErasedSegment) -> Vec<String> {
    tree.recursive_crawl_all(false)
        .into_iter()
        .filter(|segment| segment.is_type(SyntaxKind::Unparsable))
        .map(|segment| {
            format!(
                "Unparsable segment found: {} at position {:?}",
                segment.raw(),
                segment.get_position_marker()
            )
        })
        .collect()
}

fn parse_with_config(sql_path: &str, config: &Value) {
    let dialect = postgres::dialect(Some(config));

    let yaml_path = std::path::PathBuf::from(sql_path).with_extension("yml");
    let yaml_path = std::path::absolute(&yaml_path).unwrap();

    let sql = std::fs::read_to_string(sql_path).unwrap();
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    let tokens = lexer.lex(&tables, sql);
    assert!(tokens.1.is_empty());

    let parsed = parser.parse(&tables, &tokens.0).unwrap();
    let tree = parsed.unwrap();

    let unparsable_segments = check_no_unparsable_segments(&tree);
    if !unparsable_segments.is_empty() {
        panic!(
            "Found unparsable segments in {}: {}",
            sql_path,
            unparsable_segments.join(", ")
        );
    }

    let tree = tree.to_serialised(true, true);
    let actual = serde_yaml::to_string(&tree).unwrap();

    expect_file![yaml_path].assert_eq(&actual);
}

#[test]
fn postgres_pg_trgm() {
    let mut config_map = HashMap::new();
    config_map.insert("pg_trgm".to_string(), Value::Bool(true));
    let config = Value::Map(config_map);

    parse_with_config(
        "test/fixtures/dialect_configs/postgres_pg_trgm/pgtrgm.sql",
        &config,
    );
}

#[test]
fn postgres_pgvector() {
    let mut config_map = HashMap::new();
    config_map.insert("pgvector".to_string(), Value::Bool(true));
    let config = Value::Map(config_map);

    parse_with_config(
        "test/fixtures/dialect_configs/postgres_pgvector/pgvector.sql",
        &config,
    );
}

#[test]
fn postgres_pgvector_operators() {
    let mut config_map = HashMap::new();
    config_map.insert("pgvector".to_string(), Value::Bool(true));
    let config = Value::Map(config_map);

    parse_with_config(
        "test/fixtures/dialect_configs/postgres_pgvector/pgvector_operators.sql",
        &config,
    );
}
