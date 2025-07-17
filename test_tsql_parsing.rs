use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let tables = Tables::default();
    
    // Test the problematic CASE expression from select.sql
    let test_cases = vec![
        ("Simple CASE", "SELECT CASE WHEN 1 = 1 THEN 'True' END"),
        ("Full CASE from select.sql", r#"SELECT
	CASE WHEN 1 = 1 THEN 'True'
		 WHEN 1 > 1 THEN 'False'
		 WHEN 1 < 1 THEN 'False'
		 ELSE 'Silly Tests'
	END"#),
        ("BEGIN END", "BEGIN SELECT * FROM customers; END"),
        ("FOR SYSTEM_TIME BETWEEN", "SELECT * FROM Employee FOR SYSTEM_TIME BETWEEN '2021-01-01' AND '2022-01-01'"),
    ];
    
    for (name, sql) in test_cases {
        println!("\n=== Testing: {} ===", name);
        println!("SQL: {}", sql);
        
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        let (tokens, errors) = lexer.lex(&tables, sql);
        
        if !errors.is_empty() {
            println!("Lexer errors: {:?}", errors);
        }
        
        match parser.parse(&tables, &tokens, None) {
            Ok(Some(tree)) => {
                println!("✓ Parse successful!");
            }
            Ok(None) => println!("✗ Parser returned None"),
            Err(e) => println!("✗ Parse error: {:?}", e),
        }
    }
}