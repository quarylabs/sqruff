#[cfg(test)]
mod tests {
    use sqruff_lib_core::parser::parser::{Parser, Rule};
    use sqruff_lib_core::rules::get_dialect;

    #[test]
    fn test_union_with_option() {
        let dialect = get_dialect("tsql");
        let parser = Parser::new(&dialect, Default::default());
        
        let sql = "SELECT 1 UNION SELECT 2 OPTION (MERGE UNION);";
        let parsed = parser.parse(sql, None, None, None, None, None).unwrap();
        
        // Print the parse tree for debugging
        println!("Parse tree: {:#?}", parsed);
        
        // Check if the OPTION clause is parsed correctly
        let violations = parsed.find_all(Rule::any());
        let has_unparseable = violations.iter().any(|v| v.get_type() == "unparseable");
        
        assert!(!has_unparseable, "SQL should not have unparseable sections");
    }
}