#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;

    #[test]
    fn test_case_in_select_debug() {
        let dialect = crate::dialects().get(&DialectKind::Tsql).unwrap();
        let parser = Parser::from(&dialect);

        // Test 1: Simple CASE expression
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        println!("Parsing: {}", sql);
        
        let parsed = parser.parse(sql, None, None).unwrap();
        let tree = parsed.tree();
        
        println!("Parsed tree:\n{:#?}", tree);
        
        // Check if CASE is unparsable
        let unparsable_segments: Vec<_> = tree
            .recursive_crawl(&|seg| seg.is_type("unparsable"), true)
            .collect();
            
        if !unparsable_segments.is_empty() {
            println!("Found unparsable segments:");
            for seg in &unparsable_segments {
                println!("  - {:?}", seg.raw());
            }
        }
        
        // Test the SelectClauseElementSegment grammar directly
        println!("\nTesting SelectClauseElementSegment grammar:");
        let select_element_grammar = dialect.grammar("SelectClauseElementSegment");
        println!("SelectClauseElementSegment: {:?}", select_element_grammar);
    }
}