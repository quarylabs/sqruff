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
            
            // Check if parsed correctly
            let from_section = yaml.split("from_clause:").nth(1).unwrap_or("");
            let has_table_ref = from_section.contains("Users");
            let has_alias = from_section.contains("AS") && from_section.contains("u");
            let has_hint = from_section.contains("WITH") && from_section.contains("NOLOCK");
            let has_unparsable = from_section.contains("unparsable");
            
            println!("Has table ref: {}", has_table_ref);
            println!("Has alias: {}", has_alias);
            println!("Has hint: {}", has_hint);
            println!("Has unparsable: {}", has_unparsable);
            
            if has_unparsable {
                println!("FAILED: Contains unparsable sections");
            } else if has_table_ref && has_alias && has_hint {
                println!("SUCCESS: All parts parsed correctly");
            }
        }
    }
}