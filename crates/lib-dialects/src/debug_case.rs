use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use crate::kind_to_dialect;

#[test]
fn debug_case_expression() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    
    // Test SQL
    let sql = "SELECT CASE WHEN 1=1 THEN 'A' END;";
    
    // Lex the SQL
    let tables = Tables::default();
    let lexer = Lexer::from(&dialect);
    let (tokens, _errors) = lexer.lex(&tables, sql);
    
    // Print lexed tokens
    println!("\n=== LEXED TOKENS ===");
    for (i, token) in tokens.iter().enumerate() {
        let raw = token.raw();
        println!("{:3}: raw = '{}'", i, raw);
    }
    
    // Check if CASE is in reserved keywords
    println!("\n=== DIALECT KEYWORDS CHECK ===");
    let reserved = dialect.sets("reserved_keywords");
    println!("Reserved keywords contains 'CASE': {}", reserved.contains("CASE"));
    
    // Check if CASE reference exists by trying to get it
    println!("\n=== DIALECT REFERENCE CHECK ===");
    let case_ref = std::panic::catch_unwind(|| {
        dialect.r#ref("CASE")
    });
    match case_ref {
        Ok(_) => println!("'CASE' reference found in dialect"),
        Err(_) => println!("'CASE' reference NOT found in dialect!"),
    }
    
    // Parse the tokens
    let parser = Parser::from(&dialect);
    let parsed = parser.parse(&tables, &tokens, None).unwrap();
    
    println!("\n=== PARSED TREE ===");
    if let Some(tree) = parsed {
        print_tree(&tree, 0);
    }
    
    // Also test a WHERE clause CASE for comparison
    println!("\n\n=== TESTING WHERE CLAUSE CASE ===");
    let sql2 = "SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1;";
    let (tokens2, _) = lexer.lex(&tables, sql2);
    
    println!("\n=== WHERE CLAUSE LEXED TOKENS ===");
    for (i, token) in tokens2.iter().enumerate() {
        let raw = token.raw();
        if raw == "CASE" || raw == "WHEN" || raw == "THEN" || raw == "ELSE" || raw == "END" {
            println!("{:3}: raw = '{}' <-- KEYWORD", i, raw);
        }
    }
    
    let parsed2 = parser.parse(&tables, &tokens2, None).unwrap();
    println!("\n=== WHERE CLAUSE PARSED TREE ===");
    if let Some(tree) = parsed2 {
        print_tree(&tree, 0);
    }
}

fn print_tree(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
    let indent = "  ".repeat(depth);
    let raw = node.raw();
    
    // Try to get type name - for debugging we'll use a simple approach
    let type_name = if raw.is_empty() {
        "Container"
    } else {
        "Token"
    };
    
    if !raw.is_empty() {
        println!("{}{}: '{}'", indent, type_name, raw);
    } else if !node.segments().is_empty() {
        println!("{}{} (children: {})", indent, type_name, node.segments().len());
    }
    
    for child in node.segments() {
        print_tree(child, depth + 1);
    }
}