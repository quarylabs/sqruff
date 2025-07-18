use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::helpers;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_dialects::kind_to_dialect;
use std::str::FromStr;

fn main() {
    let dialect_kind = DialectKind::from_str("tsql").unwrap();
    let dialect = kind_to_dialect(&dialect_kind).unwrap();
    
    let sql = "SELECT 1;\nGO\nSELECT 2;";
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    
    let tokens = lexer.lex(&tables, sql);
    println!("Tokens: {:?}", tokens.1);
    
    let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
    let tree = parsed.unwrap();
    let tree = tree.to_serialised(true, true);
    
    println!("Parsed:\n{}", serde_yaml::to_string(&tree).unwrap());
}