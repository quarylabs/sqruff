#[cfg(test)]
mod test_tsql_case {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use crate::kind_to_dialect;

    #[test]
    fn debug_case_expression_in_select() {
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        
        // Test SQL with CASE in SELECT
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        
        // Lex
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let (tokens, _) = lexer.lex(&tables, sql);
        
        println!("\n=== TOKENS ===");
        for (i, token) in tokens.iter().enumerate() {
            println!("{}: '{}' (kind: {:?})", i, token.raw(), token.get_type());
        }
        
        // Parse
        let parser = Parser::from(&dialect);
        let parsed = parser.parse(&tables, &tokens, None).unwrap().unwrap();
        
        println!("\n=== PARSED TREE ===");
        print_tree(&parsed, 0);
        
        // Check if CASE is unparsable
        fn check_unparsable(node: &sqruff_lib_core::parser::segments::ErasedSegment) -> bool {
            if node.get_type() == sqruff_lib_core::dialects::syntax::SyntaxKind::Unparsable {
                println!("\nFOUND UNPARSABLE: {}", node.raw());
                return true;
            }
            for child in node.segments() {
                if check_unparsable(child) {
                    return true;
                }
            }
            false
        }
        
        let has_unparsable = check_unparsable(&parsed);
        assert!(!has_unparsable, "CASE expression should be parsable in SELECT clause");
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