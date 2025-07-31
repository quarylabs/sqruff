#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::Parser;
    use sqruff_lib_dialects::kind_to_dialect;

    #[test]
    fn test_if_parsing() {
        let sql = "IF 1 = 1 PRINT 'True' ELSE PRINT 'False'";
        let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
        let parser = Parser::new(dialect, Default::default());
        let (tree, errors) = parser.parse_string(sql);
        
        // Print the tree structure
        println!("Parse tree:\n{:#?}", tree);
        
        // Check for errors
        if !errors.is_empty() {
            println!("Parse errors: {:?}", errors);
        }
        
        // Check that IF is parsed as an IF statement, not object_reference
        let tree_str = format!("{:?}", tree);
        assert!(!tree_str.contains("object_reference: IF"), "IF should not be parsed as object_reference");
        assert!(tree_str.contains("if_statement") || tree_str.contains("IfStatement"), "IF should be parsed as IfStatement");
    }
}