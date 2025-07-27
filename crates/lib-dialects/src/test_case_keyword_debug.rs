#[cfg(test)]
mod test_case_keyword_debug {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use sqruff_lib_core::parser::matchable::MatchableTrait;
    use crate::kind_to_dialect;
    use ahash::AHashMap;

    #[test]
    fn debug_case_keyword_matching() {
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        
        // Check if CASE is in the dialect's references
        let case_ref = dialect.r#ref("CASE");
        println!("\nCASE reference exists in dialect");
        
        // Test simple CASE keyword matching
        let sql = "CASE";
        
        // Lex
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let (tokens, _) = lexer.lex(&tables, sql);
        
        println!("\n=== TOKENS ===");
        for token in &tokens {
            if !token.raw().trim().is_empty() {
                println!("'{}' (kind: {:?})", token.raw(), token.get_type());
            }
        }
        
        // Parse just the keyword
        let parser = Parser::from(&dialect);
        let parsed = parser.parse(&tables, &tokens, None).unwrap().unwrap();
        
        println!("\n=== PARSED TREE ===");
        print_tree(&parsed, 0);
        
        // Now test parsing CASE as a keyword reference
        let case_ref = dialect.r#ref("CASE");
        let indentation_config = AHashMap::new();
        let mut parse_context = sqruff_lib_core::parser::context::ParseContext::new(&dialect, &indentation_config);
        
        // Get parsed segments from the tree
        let segments = parsed.segments();
        if !segments.is_empty() {
            println!("\n=== TESTING CASE KEYWORD MATCH ===");
            match case_ref.match_segments(&segments, 0, &mut parse_context) {
                Ok(result) => {
                    if result.has_match() {
                        println!("SUCCESS: CASE keyword matched!");
                        println!("Match span: {:?}", result.span);
                    } else {
                        println!("FAILED: CASE keyword did not match");
                    }
                }
                Err(e) => {
                    println!("ERROR: {:?}", e);
                }
            }
        }
    }
    
    fn print_tree(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
        let indent = "  ".repeat(depth);
        let raw = node.raw();
        let kind = node.get_type();
        
        if !raw.is_empty() {
            println!("{}{:?}: '{}'", indent, kind, raw);
        } else if !node.segments().is_empty() {
            println!("{}{:?} (children: {})", indent, kind, node.segments().len());
        }
        
        for child in node.segments() {
            print_tree(child, depth + 1);
        }
    }
}