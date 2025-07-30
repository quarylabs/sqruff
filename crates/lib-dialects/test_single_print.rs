use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use sqruff_lib_dialects::kind_to_dialect;

fn main() {
    let sql = "PRINT 'test'";
    
    let dialect_kind = DialectKind::Tsql;
    let dialect = kind_to_dialect(&dialect_kind).unwrap();
    
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    
    println!("SQL: {}", sql);
    println!("Dialect: {:?}", dialect_kind);
    
    let tokens = lexer.lex(&tables, sql.into());
    println!("Lexer errors: {:?}", tokens.1);
    println!("Tokens: {:?}", tokens.0);
    
    if !tokens.1.is_empty() {
        println!("Lexer failed!");
        return;
    }
    
    let parsed = parser.parse(&tables, &tokens.0, None);
    match parsed {
        Ok(Some(tree)) => {
            println!("Parse successful!");
            let serialized = tree.to_serialised(true, true);
            println!("Tree: {:#?}", serialized);
            
            // Also output as YAML like the test does
            let yaml = serde_yaml::to_string(&serialized).unwrap();
            println!("YAML:\n{}", yaml);
        }
        Ok(None) => {
            println!("Parse returned None");
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}