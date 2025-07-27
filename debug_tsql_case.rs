// Debug test to check CASE parsing in T-SQL
#[cfg(test)]
mod tests {
    use sqruff_lib_core::dialects::init::DialectKind;
    use sqruff_lib_core::parser::parser::Parser;
    
    #[test]
    fn test_tsql_case_parsing() {
        let dialect = DialectKind::Tsql.new_dialect();
        let parser = Parser::new(&dialect, Default::default());
        
        // Test 1: Simple CASE in SELECT
        let sql = "SELECT CASE WHEN 1=1 THEN 'A' END";
        let parsed = parser.parse(sql, None).unwrap();
        
        // Print the parse tree
        println!("Parse tree for: {}", sql);
        println!("{:#?}", parsed);
        
        // Test 2: CASE with alias
        let sql2 = "SELECT CASE WHEN 1=1 THEN 'A' END AS test";
        let parsed2 = parser.parse(sql2, None).unwrap();
        
        println!("\nParse tree for: {}", sql2);
        println!("{:#?}", parsed2);
        
        // Test 3: T-SQL equals alias
        let sql3 = "SELECT test = CASE WHEN 1=1 THEN 'A' END";
        let parsed3 = parser.parse(sql3, None).unwrap();
        
        println!("\nParse tree for: {}", sql3);
        println!("{:#?}", parsed3);
    }
}