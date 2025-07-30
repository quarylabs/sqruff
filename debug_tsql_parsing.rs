use std::str::FromStr;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_dialects::kind_to_dialect;

fn main() {
    let dialect_kind = DialectKind::from_str("tsql").unwrap();
    let dialect = kind_to_dialect(&dialect_kind).unwrap();
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);

    // Test simple PRINT statement
    println!("=== Testing simple PRINT statement ===");
    let sql = "PRINT 'test'";
    let tokens = lexer.lex(&tables, sql.to_string());
    println!("Tokens: {:?}", tokens.0);
    if !tokens.1.is_empty() {
        println!("Lexer errors: {:?}", tokens.1);
    }
    
    let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
    if let Some(tree) = parsed {
        let tree = tree.to_serialised(true, true);
        println!("Parse tree: {}", serde_yaml::to_string(&tree).unwrap());
    } else {
        println!("Failed to parse!");
    }

    // Test IF statement
    println!("\n=== Testing simple IF statement ===");
    let sql = "IF @nm IS NULL PRINT 'test'";
    let tokens = lexer.lex(&tables, sql.to_string());
    println!("Tokens: {:?}", tokens.0);
    if !tokens.1.is_empty() {
        println!("Lexer errors: {:?}", tokens.1);
    }
    
    let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
    if let Some(tree) = parsed {
        let tree = tree.to_serialised(true, true);
        println!("Parse tree: {}", serde_yaml::to_string(&tree).unwrap());
    } else {
        println!("Failed to parse!");
    }

    // Test procedure definition (simplified)
    println!("\n=== Testing procedure body part ===");
    let sql = "IF @nm IS NULL\n    BEGIN\n        PRINT 'You must give a user name'\n        RETURN\n    END";
    let tokens = lexer.lex(&tables, sql.to_string());
    println!("Tokens: {:?}", tokens.0);
    if !tokens.1.is_empty() {
        println!("Lexer errors: {:?}", tokens.1);
    }
    
    let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
    if let Some(tree) = parsed {
        let tree = tree.to_serialised(true, true);
        println!("Parse tree: {}", serde_yaml::to_string(&tree).unwrap());
    } else {
        println!("Failed to parse!");
    }
}