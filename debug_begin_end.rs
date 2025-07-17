use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let tables = Tables::default();
    
    let test_cases = vec![
        ("Simple BEGIN END", "BEGIN SELECT * FROM customers; END"),
        ("BEGIN with transaction", "BEGIN TRANSACTION SELECT * FROM customers; COMMIT"),
        ("Wrapped in statement", "DECLARE @x INT; BEGIN SELECT * FROM customers; END"),
        ("Multiple statements", "BEGIN SELECT * FROM customers; UPDATE customers SET status = 'Active'; END"),
    ];
    
    for (name, sql) in test_cases {
        println!("\n=== Testing: {} ===", name);
        println!("SQL: {}", sql);
        
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        let (tokens, errors) = lexer.lex(&tables, sql);
        
        if !errors.is_empty() {
            println!("Lexer errors: {:?}", errors);
            continue;
        }
        
        println!("Tokens: {:?}", tokens.iter().map(|t| t.raw()).collect::<Vec<_>>());
        
        match parser.parse(&tables, &tokens, None) {
            Ok(Some(tree)) => {
                println!("✓ Parse successful!");
                // Print first few levels of tree structure
                let serialized = tree.to_serialised(true, true);
                if let serde_yaml::Value::Mapping(map) = &serialized {
                    if let Some(serde_yaml::Value::Sequence(seq)) = map.get("file") {
                        println!("File contains {} top-level elements", seq.len());
                        for (i, item) in seq.iter().take(3).enumerate() {
                            println!("  [{}]: {:?}", i, item);
                        }
                    }
                }
            }
            Ok(None) => println!("✗ Parser returned None"),
            Err(e) => println!("✗ Parse error: {:?}", e),
        }
    }
}