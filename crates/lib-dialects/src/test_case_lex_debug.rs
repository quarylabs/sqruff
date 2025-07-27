#[cfg(test)]
mod tests {
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use crate::tsql;

    #[test]
    fn test_case_lexing() {
        let dialect = tsql::dialect();
        let parser = Parser::from(&dialect);
        let lexer = Lexer::from(&dialect);
        let tables = Tables::default();
        
        // Test parsing "CASE" in simple SELECT
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        let (tokens, lex_errors) = lexer.lex(&tables, sql);
        assert!(lex_errors.is_empty());
        
        // Show lexed tokens
        println!("SQL: {}", sql);
        println!("Tokens:");
        for (i, token) in tokens.iter().enumerate() {
            println!("  {}: {:?} ({})", i, token.get_type(), token.raw());
        }
        
        let parsed = parser.parse(&tables, &tokens, None).unwrap();
        
        if let Some(tree) = parsed {
            // Look for unparsable segments
            let unparsable: Vec<_> = tree
                .recursive_crawl(&|seg| seg.is_type("unparsable"), true)
                .collect();
                
            println!("\nUnparsable segments: {}", unparsable.len());
            
            if !unparsable.is_empty() {
                println!("Unparsable content:");
                for seg in &unparsable {
                    println!("  - {:?}", seg.raw());
                }
                
                // Show the tree structure around CASE
                println!("\nTree structure:");
                let select_clause = tree
                    .recursive_crawl(&|seg| seg.is_type("select_clause"), true)
                    .next();
                    
                if let Some(sc) = select_clause {
                    for child in sc.segments() {
                        println!("  Select clause child: {:?} ({})", child.get_type(), child.raw());
                    }
                }
            }
        } else {
            println!("Parse failed!");
        }
        
        // Check if CASE is in reserved keywords
        let reserved = dialect.sets("reserved_keywords");
        println!("\nIs CASE in reserved keywords? {}", reserved.contains(&"CASE".into()));
        
        // Check the keyword mapping
        let case_keyword = dialect.r#ref("CASE");
        println!("\nRef('CASE'): {:?}", case_keyword);
    }
}