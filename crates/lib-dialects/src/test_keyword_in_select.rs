#[cfg(test)]
mod test_keyword_in_select {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use crate::kind_to_dialect;

    #[test]
    fn test_case_keyword_recognition_in_select() {
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        
        // Test SQL with CASE in SELECT - should have keywords
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        
        // Lex
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let (tokens, _) = lexer.lex(&tables, sql);
        
        println!("\n=== TOKENS IN SELECT CLAUSE ===");
        for (i, token) in tokens.iter().enumerate() {
            if !token.raw().trim().is_empty() {
                println!("{}: '{}' (kind: {:?})", i, token.raw(), token.get_type());
            }
        }
        
        // Test SQL with CASE in WHERE - for comparison
        let sql2 = "SELECT col FROM table WHERE CASE WHEN 1=1 THEN 'A' END = 'A'";
        let (tokens2, _) = lexer.lex(&tables, sql2);
        
        println!("\n=== TOKENS IN WHERE CLAUSE ===");
        let mut in_where = false;
        for (i, token) in tokens2.iter().enumerate() {
            if token.raw() == "WHERE" {
                in_where = true;
            }
            if in_where && !token.raw().trim().is_empty() {
                println!("{}: '{}' (kind: {:?})", i, token.raw(), token.get_type());
            }
        }
    }
}