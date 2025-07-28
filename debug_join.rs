use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_dialects::tsql;

fn main() {
    let dialect = tsql::dialect();
    let parser = Parser::new(&dialect, Default::default());
    
    // Test 1: Simple join
    let sql1 = "SELECT * FROM table1 INNER JOIN table2 ON table1.id = table2.id";
    println!("Test 1: {}", sql1);
    let result1 = parser.parse_string(sql1, None, None, None, None);
    match result1 {
        Ok(parsed) => println!("Success"),
        Err(e) => println!("Error: {:?}", e),
    }
    
    // Test 2: Join with hint
    let sql2 = "SELECT * FROM table1 INNER HASH JOIN table2 ON table1.id = table2.id";
    println!("\nTest 2: {}", sql2);
    let result2 = parser.parse_string(sql2, None, None, None, None);
    match result2 {
        Ok(parsed) => println!("Success"),
        Err(e) => println!("Error: {:?}", e),
    }
    
    // Test 3: FULL OUTER MERGE JOIN
    let sql3 = "SELECT * FROM table1 FULL OUTER MERGE JOIN table2 ON table1.id = table2.id";
    println!("\nTest 3: {}", sql3);
    let result3 = parser.parse_string(sql3, None, None, None, None);
    match result3 {
        Ok(parsed) => println!("Success"),
        Err(e) => println!("Error: {:?}", e),
    }
}