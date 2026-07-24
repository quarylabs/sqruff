use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_dialects::kind_to_dialect;

/// A parenthesised function call in the Oracle dialect used to panic because
/// FunctionContentsGrammar referenced a segment that was never defined. It must
/// now parse and round-trip losslessly instead of crashing.
#[test]
fn oracle_parses_function_call() {
    let dialect = kind_to_dialect(&DialectKind::Oracle, None).unwrap();
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    let sql = "select upper(col1), count(*) from my_table";
    let tokens = lexer.lex(&tables, sql.to_string());
    let tree = parser.parse(&tables, &tokens.0).unwrap().unwrap();
    assert_eq!(tree.raw().as_str(), sql);
}
