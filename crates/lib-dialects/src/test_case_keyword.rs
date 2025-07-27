#[cfg(test)]
mod test_case_keyword {
    use crate::tsql;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use sqruff_lib_core::dialects::syntax::SyntaxKind;
    
    #[test]
    fn test_case_keyword_matching() {
        let dialect = tsql::dialect();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        let tables = Tables::default();
        
        // Test 1: Check if CASE is in reserved keywords
        let reserved = dialect.sets("reserved_keywords");
        println!("\nCASE in reserved keywords: {}", reserved.contains("CASE"));
        
        // Test 2: Lex just "CASE" and check token type
        let (tokens, _) = lexer.lex(&tables, "CASE");
        for token in &tokens {
            if !token.raw().is_empty() {
                println!("Token: {:?} '{}' (is_keyword: {})", 
                    token.get_type(), 
                    token.raw(),
                    token.is_type(SyntaxKind::Keyword)
                );
            }
        }
        
        // Test 3: Create a simple CASE expression and parse it
        println!("\n=== CASE Expression Parsing ===");
        let case_sql = "CASE WHEN 1=1 THEN 2 END";
        let (case_tokens, _) = lexer.lex(&tables, case_sql);
        
        // Try to parse the CASE expression
        match parser.parse(&tables, &case_tokens, None) {
            Ok(Some(tree)) => {
                println!("CASE expression parse: SUCCESS");
                // Check if it contains CaseExpression node
                let has_case_expr = check_for_case_expression(&tree);
                println!("Contains CaseExpression node: {}", has_case_expr);
            },
            Ok(None) => {
                println!("CASE expression parse: NO RESULT");
            },
            Err(e) => {
                println!("CASE expression parse: ERROR - {:?}", e);
            }
        }
        
        // Test 4: Check how keywords are handled
        println!("\n=== Keyword Processing ===");
        // Try just the word "SELECT" to compare
        let (select_tokens, _) = lexer.lex(&tables, "SELECT");
        for token in &select_tokens {
            if !token.raw().is_empty() {
                println!("SELECT token: {:?} (is_keyword: {})", 
                    token.get_type(), 
                    token.is_type(SyntaxKind::Keyword)
                );
            }
        }
    }
    
    fn check_for_case_expression(node: &sqruff_lib_core::parser::segments::ErasedSegment) -> bool {
        if node.get_type() == SyntaxKind::CaseExpression {
            return true;
        }
        for child in node.segments() {
            if check_for_case_expression(child) {
                return true;
            }
        }
        false
    }
}