#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_core::parser::lexer::{Lexer, StringOrTemplate};
    use sqruff_lib_core::parser::segments::Tables;
    use crate::kind_to_dialect;

    #[test]
    fn test_with_nolock_parsing() {
        let dialect_kind = DialectKind::Tsql;
        let dialect = kind_to_dialect(&dialect_kind).unwrap();
        
        // Start with simple case
        let sql = "SELECT * FROM Users WITH(NOLOCK)";
        println!("\nTesting simple case: {}", sql);
        
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        
        let tokens = lexer.lex(&tables, StringOrTemplate::String(sql)).unwrap();
        println!("Token count: {}", tokens.0.len());
        
        let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
        
        if let Some(tree) = parsed {
            // Print the parse tree in YAML format for debugging
            let yaml = serde_yaml::to_string(&tree.to_serialised(true, true)).unwrap();
            
            // Check for specific parsing issues
            if yaml.contains("unparsable") {
                println!("FAILED: Query contains unparsable sections");
                // Print just the relevant part
                for line in yaml.lines() {
                    if line.contains("FROM") || line.contains("unparsable") || line.contains("WITH") || line.contains("table") {
                        println!("{}", line);
                    }
                }
            } else if yaml.contains("table_hint") || (yaml.contains("- keyword: WITH") && yaml.contains("- keyword: NOLOCK")) {
                println!("SUCCESS: WITH(NOLOCK) parsed correctly");
            } else {
                println!("UNCLEAR: Check full parse tree");
                println!("{}", yaml);
            }
        }
    }
    
    #[test]
    fn test_with_alias_nolock() {
        let dialect_kind = DialectKind::Tsql;
        let dialect = kind_to_dialect(&dialect_kind).unwrap();
        
        let sql = "SELECT * FROM Users AS u WITH(NOLOCK)";
        println!("\nTesting with alias: {}", sql);
        
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        
        let tokens = lexer.lex(&tables, StringOrTemplate::String(sql)).unwrap();
        let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
        
        if let Some(tree) = parsed {
            let yaml = serde_yaml::to_string(&tree.to_serialised(true, true)).unwrap();
            println!("Full parse tree:\n{}", yaml);
            
            // Check if parsed correctly
            let has_unparsable = yaml.contains("unparsable");
            
            if has_unparsable {
                println!("FAILED: Contains unparsable sections");
            } else {
                println!("SUCCESS: Parsed without unparsable sections");
            }
        }
    }
    
    #[test]
    fn test_simple_join_with_nolock() {
        let dialect_kind = DialectKind::Tsql;
        let dialect = kind_to_dialect(&dialect_kind).unwrap();
        
        let sql = "SELECT * FROM Users u WITH(NOLOCK) JOIN Orders o WITH(NOLOCK) ON u.id = o.user_id";
        println!("\nTesting simple JOIN with NOLOCK: {}", sql);
        
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        
        let tokens = lexer.lex(&tables, StringOrTemplate::String(sql)).unwrap();
        let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
        
        if let Some(tree) = parsed {
            let yaml = serde_yaml::to_string(&tree.to_serialised(true, true)).unwrap();
            
            let unparsable_count = yaml.matches("unparsable").count();
            if unparsable_count > 0 {
                println!("FAILED: Found {} unparsable sections", unparsable_count);
                println!("\nFull parse tree:");
                println!("{}", yaml);
            } else {
                println!("SUCCESS: JOIN with NOLOCK parsed correctly!");
                // Also print to verify structure
                println!("\nSuccessful parse tree:");
                for line in yaml.lines() {
                    if line.contains("from_") || line.contains("join") || line.contains("WITH") || line.contains("NOLOCK") || line.contains("table") {
                        println!("{}", line);
                    }
                }
            }
        }
    }
    
    #[test]
    fn test_basic_from() {
        let dialect_kind = DialectKind::Tsql;
        let dialect = kind_to_dialect(&dialect_kind).unwrap();
        
        let sql = "SELECT * FROM Users";
        println!("\nTesting basic FROM: {}", sql);
        
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        
        let tokens = lexer.lex(&tables, StringOrTemplate::String(sql)).unwrap();
        let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
        
        if let Some(tree) = parsed {
            let yaml = serde_yaml::to_string(&tree.to_serialised(true, true)).unwrap();
            
            if yaml.contains("unparsable") {
                println!("FAILED: Basic FROM clause is unparsable!");
                println!("{}", yaml);
            } else {
                println!("SUCCESS: Basic FROM clause works");
            }
        }
    }
    
    #[test]
    fn test_al05_exact_issue() {
        let dialect_kind = DialectKind::Tsql;
        let dialect = kind_to_dialect(&dialect_kind).unwrap();
        
        let sql = "SELECT COUNT(*) FROM schema1.Table_Sales_Position_Reference AS op2ref WITH(NOLOCK) INNER JOIN schema1.TBL_POS_DATA AS Position WITH(NOLOCK) ON Position.I_POS_ID = op2ref.i_position_id WHERE op2ref.i_referencetype_id = 1;";
        println!("\nTesting AL05 exact issue: {}", sql);
        
        let tables = Tables::default();
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        
        let tokens = lexer.lex(&tables, StringOrTemplate::String(sql)).unwrap();
        let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
        
        if let Some(tree) = parsed {
            let yaml = serde_yaml::to_string(&tree.to_serialised(true, true)).unwrap();
            
            // Look for unparsable sections
            let unparsable_count = yaml.matches("unparsable").count();
            println!("Unparsable sections found: {}", unparsable_count);
            
            if unparsable_count > 0 {
                println!("FAILED: Found {} unparsable sections", unparsable_count);
                // Print context around unparsable sections
                let lines: Vec<&str> = yaml.lines().collect();
                for (i, line) in lines.iter().enumerate() {
                    if line.contains("unparsable") {
                        // Print 5 lines before and after
                        let start = if i >= 5 { i - 5 } else { 0 };
                        let end = if i + 10 < lines.len() { i + 10 } else { lines.len() };
                        println!("\nContext around unparsable (line {}):", i);
                        for j in start..end {
                            if j == i {
                                println!(">>> {}", lines[j]);
                            } else {
                                println!("    {}", lines[j]);
                            }
                        }
                    }
                }
            } else {
                println!("SUCCESS: No unparsable sections!");
            }
        }
    }
}