use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::tsql;

fn main() {
    let sql = "EXECUTE test WITH RESULT SETS ((col1 INT));";
    
    let mut tsql_dialect = tsql();
    
    // Parse the SQL
    match tsql_dialect.parse(sql, None) {
        Ok(parse_result) => {
            println!("Parse successful!");
            if let Some(tree) = parse_result.tree {
                println!("Tree structure:");
                println!("{:#?}", tree);
            }
            
            if !parse_result.violations.is_empty() {
                println!("\nViolations found:");
                for violation in &parse_result.violations {
                    println!("- {}", violation);
                }
            }
        }
        Err(e) => {
            println!("Parse failed: {:?}", e);
        }
    }
}