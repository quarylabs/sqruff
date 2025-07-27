#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::parsers::StringParser;
    use sqruff_lib_core::parser::segments::test_functions::fresh_ansi_dialect;

    #[test]
    fn test_case_parsing_issue() {
        let dialect = crate::dialects().get(&DialectKind::Tsql).unwrap();
        let parser = Parser::from(&dialect);
        
        // Test parsing just "CASE" to see what happens
        let segments = parser.lex("CASE").unwrap();
        println!("Lexed CASE: {:?}", segments);
        
        // Get the SelectClauseElementSegment grammar
        let select_element = dialect.grammar("SelectClauseElementSegment");
        
        // Try to match against just "CASE"
        let ctx = parser.context.clone();
        let (matched, remaining) = select_element.match_segments(&segments, &ctx).unwrap();
        
        println!("Matched: {:?}", matched);
        println!("Remaining: {:?}", remaining);
        
        // Now test the full SELECT CASE expression
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        let parsed = parser.parse(sql, None, None).unwrap();
        
        // Find unparsable segments
        let unparsable: Vec<_> = parsed.tree()
            .recursive_crawl(&|seg| seg.is_type("unparsable"), true)
            .collect();
            
        println!("\nUnparsable segments in '{}': {}", sql, unparsable.len());
        for seg in &unparsable {
            println!("  - {:?}", seg.raw());
        }
    }
}