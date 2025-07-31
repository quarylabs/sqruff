use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_dialects::kind_to_dialect;
use std::str::FromStr;

fn main() {
    let dialect_kind = DialectKind::from_str("tsql").unwrap();
    let dialect = kind_to_dialect(&dialect_kind).unwrap();
    
    let sql = "GO\n";
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    
    println!("Lexing SQL: {:?}", sql);
    let tokens = lexer.lex(&tables, sql);
    println!("Lexing errors: {:?}", tokens.1);
    println!("Token count: {}", tokens.0.len());
    
    println!("Parsing tokens...");
    let parsed = parser.parse(&tables, &tokens.0, None);
    
    match parsed {
        Ok(Some(tree)) => {
            println!("SUCCESS: Parsed GO successfully!");
            let serialized = tree.to_serialised(true, true);
            println!("Parse tree: {:#?}", serialized);
        }
        Ok(None) => {
            println!("RESULT: Empty parse result");
        }
        Err(error) => {
            println!("ERROR: Parse failed: {:#?}", error);
        }
    }
}