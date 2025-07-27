#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::parser::Parser;
    use sqruff_lib::Config;

    #[test]
    fn test_case_expression_parsing() {
        // Test SQL with CASE expression in SELECT
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END;";
        
        // Create config with T-SQL dialect
        let mut config = Config::default();
        config.dialect = DialectKind::Tsql;
        
        // Parse the SQL
        let parser = Parser::new(&config, None, true);
        let tables = parser.parse(sql, None, None, None).unwrap();
        let parsed = tables.tree().unwrap();
        
        // Print the parsed tree
        println!("\n\nParsed tree for: {}", sql);
        println!("{:#?}", parsed);
        
        // Look for unparsable segments
        fn find_unparsable(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
            let indent = "  ".repeat(depth);
            if node.raw().is_empty() && node.segments().is_empty() {
                return;
            }
            
            use sqruff_lib_core::dialects::syntax::SyntaxKind;
            let syntax_kind = node.get_syntax_kind();
            let raw = node.raw();
            
            if !raw.is_empty() || syntax_kind == SyntaxKind::Unparsable {
                println!("{}{:?}: '{}'", indent, syntax_kind, raw);
                
                if syntax_kind == SyntaxKind::Unparsable {
                    println!("{}  ^ UNPARSABLE!", indent);
                }
            }
            
            for child in node.segments() {
                find_unparsable(child, depth + 1);
            }
        }
        
        println!("\n\nTraversing tree to find issues:");
        find_unparsable(&parsed, 0);
        
        // Now test WHERE clause CASE (which should work)
        let sql2 = "SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1;";
        let tables2 = parser.parse(sql2, None, None, None).unwrap();
        let parsed2 = tables2.tree().unwrap();
        
        println!("\n\nParsed tree for WHERE clause CASE: {}", sql2);
        println!("{:#?}", parsed2);
        
        println!("\n\nTraversing WHERE clause tree:");
        find_unparsable(&parsed2, 0);
    }
}

fn main() {
    println!("Run with: cargo test --bin test_case_debug -- --nocapture");
}