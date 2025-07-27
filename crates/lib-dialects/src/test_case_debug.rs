#[cfg(test)]
mod test_case_debug {
    use crate::tsql;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use sqruff_lib_core::dialects::syntax::SyntaxKind;
    
    #[test]
    fn debug_case_expression_parsing() {
        let dialect = tsql::dialect();
        let parser = Parser::from(&dialect);
        let lexer = Lexer::from(&dialect);
        let tables = Tables::default();
        
        // Test 1: CASE in SELECT clause
        let sql = "SELECT CASE WHEN Status = 'Active' THEN 'A' END AS StatusCode";
        println!("\n=== Testing CASE in SELECT clause ===");
        println!("SQL: {}", sql);
        
        let (tokens, _) = lexer.lex(&tables, sql);
        let parsed = parser.parse(&tables, &tokens, None).unwrap().unwrap();
        
        // Print the AST
        print_ast(&parsed, 0);
        
        // Check for unparsable sections
        let unparsable_count = count_unparsable(&parsed);
        println!("\nUnparsable sections found: {}", unparsable_count);
        
        // Test 2: CASE in WHERE clause (should work)
        let sql2 = "SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1";
        println!("\n=== Testing CASE in WHERE clause ===");
        println!("SQL: {}", sql2);
        
        let (tokens2, _) = lexer.lex(&tables, sql2);
        let parsed2 = parser.parse(&tables, &tokens2, None).unwrap().unwrap();
        
        // Print the AST
        print_ast(&parsed2, 0);
        
        // Check for unparsable sections
        let unparsable_count2 = count_unparsable(&parsed2);
        println!("\nUnparsable sections found: {}", unparsable_count2);
        
        // Print detailed token info for SELECT clause tokens
        println!("\n=== Token Analysis for SELECT Clause ===");
        for (i, token) in tokens.iter().enumerate() {
            println!("Token {}: {:?} '{}' ({:?})", 
                i, 
                token.get_type(), 
                token.raw(),
                token.get_type()
            );
        }
    }
    
    fn print_ast(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
        let indent = "  ".repeat(depth);
        let raw = node.raw();
        
        if !raw.is_empty() {
            println!("{}{:?}: '{}'", indent, node.get_type(), raw);
        } else if node.segments().len() > 0 {
            println!("{}{:?}", indent, node.get_type());
            for child in node.segments() {
                print_ast(child, depth + 1);
            }
        }
    }
    
    fn count_unparsable(node: &sqruff_lib_core::parser::segments::ErasedSegment) -> usize {
        let mut count = 0;
        if node.get_type() == SyntaxKind::Unparsable {
            count += 1;
        }
        for child in node.segments() {
            count += count_unparsable(child);
        }
        count
    }
}