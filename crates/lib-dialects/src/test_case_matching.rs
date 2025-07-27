#[cfg(test)]
mod test_case_matching {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use crate::kind_to_dialect;

    #[test]
    fn debug_why_case_fails_in_select() {
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        
        // Create a minimal SELECT with CASE
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END;";
        
        // Lex and parse
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let (tokens, _) = lexer.lex(&tables, sql);
        let parser = Parser::from(&dialect);
        let parsed = parser.parse(&tables, &tokens, None).unwrap().unwrap();
        
        // Find the SelectClauseElementSegment
        fn find_select_clause_element(node: &sqruff_lib_core::parser::segments::ErasedSegment, path: &str) {
            use sqruff_lib_core::dialects::syntax::SyntaxKind;
            
            let raw = node.raw();
            let segments = node.segments();
            
            // Check if this is a SelectClauseElement by checking raw content patterns
            if raw.contains("CASE") && path.contains("select") {
                    println!("\nFound SelectClauseElement at path: {}", path);
                    println!("Raw content: '{}'", raw);
                    println!("Number of children: {}", segments.len());
                    
                    // Print children
                    for (i, child) in segments.iter().enumerate() {
                        let child_raw = child.raw();
                        println!("  Child {}: raw='{}'", i, child_raw);
                    }
            }
            
            // Check for unparsable segments
            if raw == "CASE WHEN 1=1 THEN 'A'" || raw.contains("CASE") {
                println!("\nFound segment containing CASE at path: {}", path);
                println!("Raw: '{}'", raw);
                println!("Number of segments: {}", segments.len());
            }
            
            // Recurse into children
            for (i, child) in segments.iter().enumerate() {
                find_select_clause_element(child, &format!("{}/{}", path, i));
            }
        }
        
        println!("\n=== SEARCHING FOR SELECT CLAUSE ELEMENT ===");
        find_select_clause_element(&parsed, "root");
        
        // Also check what BaseExpressionElementGrammar would match
        println!("\n=== CHECKING GRAMMAR MATCHING ===");
        
        // Try to match just "CASE WHEN 1=1 THEN 'A' END" with BaseExpressionElementGrammar
        let case_sql = "CASE WHEN 1=1 THEN 'A' END";
        let (case_tokens, _) = lexer.lex(&tables, case_sql);
        
        println!("\nTokens for '{}': ", case_sql);
        for (i, token) in case_tokens.iter().enumerate() {
            println!("  {}: '{}'", i, token.raw());
        }
        
        // Try parsing just the CASE expression
        let case_parsed = parser.parse(&tables, &case_tokens, None);
        match case_parsed {
            Ok(Some(tree)) => {
                println!("\nSuccessfully parsed CASE expression alone");
                print_simple_tree(&tree, 0);
            }
            Ok(None) => println!("\nNo parse result for CASE expression"),
            Err(e) => println!("\nError parsing CASE expression: {:?}", e),
        }
    }
    
    fn print_simple_tree(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
        let indent = "  ".repeat(depth);
        let raw = node.raw();
        
        if !raw.is_empty() {
            println!("{}Token: '{}'", indent, raw);
        } else if !node.segments().is_empty() {
            println!("{}Container (children: {})", indent, node.segments().len());
        }
        
        for child in node.segments() {
            print_simple_tree(child, depth + 1);
        }
    }
}