#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::helpers::ToMatchable;
    use sqruff_lib_core::parser::grammar::anyof::one_of;
    use sqruff_lib_core::parser::grammar::Ref;
    use sqruff_lib_core::vec_of_erased;

    #[test]
    fn test_case_parse_debug() {
        let dialect = crate::dialects().get(&DialectKind::Tsql).unwrap();
        let parser = Parser::from(&dialect);
        
        // Test what happens when we try to match "CASE"
        let segments = parser.lex("CASE").unwrap();
        println!("Lexed CASE: {:?}", segments);
        
        // Test the NakedIdentifierSegment with exclusions
        let naked_id_with_exclusions = Ref::new("NakedIdentifierSegment")
            .exclude(one_of(vec_of_erased![
                Ref::keyword("CASE"),
                Ref::keyword("CAST"),
                Ref::keyword("EXISTS"),
                Ref::keyword("NOT"),
                Ref::keyword("NULL"),
                Ref::keyword("SELECT"),
                Ref::keyword("WITH")
            ]));
            
        let ctx = parser.context.clone();
        let (matched, remaining) = naked_id_with_exclusions.match_segments(&segments, &ctx).unwrap();
        
        println!("\nNakedIdentifier with exclusions:");
        println!("  Matched: {:?}", matched);
        println!("  Remaining: {:?}", remaining);
        
        // Now test BaseExpressionElementGrammar
        let base_expr = dialect.grammar("BaseExpressionElementGrammar");
        let (matched2, remaining2) = base_expr.match_segments(&segments, &ctx).unwrap();
        
        println!("\nBaseExpressionElementGrammar:");
        println!("  Matched: {:?}", matched2);
        println!("  Remaining: {:?}", remaining2);
        
        // Test the full SelectClauseElementSegment
        let select_elem = dialect.grammar("SelectClauseElementSegment");
        let sql = "CASE WHEN 1=1 THEN 'A' END";
        let segments_full = parser.lex(sql).unwrap();
        let (matched3, remaining3) = select_elem.match_segments(&segments_full, &ctx).unwrap();
        
        println!("\nSelectClauseElementSegment on '{}':", sql);
        println!("  Matched: {:?}", matched3);
        println!("  Remaining: {:?}", remaining3);
    }
}