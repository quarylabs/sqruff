#[cfg(test)]
mod test_select_case {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::Lexer;
    use sqruff_lib_core::parser::segments::Tables;
    use sqruff_lib_core::parser::matchable::MatchableTrait;
    use ahash::AHashMap;
    use crate::kind_to_dialect;

    #[test]
    fn debug_select_clause_parsing() {
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        
        // Test just the SELECT clause content
        let sqls = vec![
            "CASE WHEN 1=1 THEN 'A' END",
            "CASE WHEN 1=1 THEN 'A' END AS StatusCode",
            "col1, CASE WHEN 1=1 THEN 'A' END",
        ];
        
        for sql in sqls {
            println!("\n=== Testing: {} ===", sql);
            
            let tables = Tables::default();
            let lexer = Lexer::from(&dialect);
            let (tokens, _) = lexer.lex(&tables, sql);
            
            println!("Tokens:");
            for (i, token) in tokens.iter().enumerate() {
                println!("  {}: '{}'", i, token.raw());
            }
            
            // Try to parse as BaseExpressionElementGrammar
            let base_expr = dialect.r#ref("BaseExpressionElementGrammar");
            let parser = Parser::from(&dialect);
            
            // Create a parse context and try to match
            let template_info = AHashMap::new();
            let mut parse_context = sqruff_lib_core::parser::context::ParseContext::new(&dialect, &template_info);
            let result = base_expr.match_segments(&tokens, 0, &mut parse_context);
            
            match result {
                Ok(match_result) => {
                    if match_result.span.start == match_result.span.end {
                        println!("BaseExpressionElementGrammar: NO MATCH");
                    } else {
                        let matched_count = (match_result.span.end - match_result.span.start) as usize;
                        println!("BaseExpressionElementGrammar: MATCHED {} tokens", matched_count);
                        for i in match_result.span.start..match_result.span.end {
                            println!("  - '{}'", tokens[i as usize].raw());
                        }
                    }
                }
                Err(e) => println!("BaseExpressionElementGrammar: ERROR {:?}", e),
            }
            
            // Also try CaseExpressionSegment directly
            let case_expr = dialect.r#ref("CaseExpressionSegment");
            let result = case_expr.match_segments(&tokens, 0, &mut parse_context);
            
            match result {
                Ok(match_result) => {
                    if match_result.span.start == match_result.span.end {
                        println!("CaseExpressionSegment: NO MATCH");
                    } else {
                        let matched_count = (match_result.span.end - match_result.span.start) as usize;
                        println!("CaseExpressionSegment: MATCHED {} tokens", matched_count);
                    }
                }
                Err(e) => println!("CaseExpressionSegment: ERROR {:?}", e),
            }
        }
    }
}