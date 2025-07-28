use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_dialects::tsql;

fn main() {
    let dialect = tsql::dialect();
    let sql = "SELECT * FROM table1 FULL OUTER MERGE JOIN table2 ON table1.id = table2.id;";
    
    let parsed = dialect.parse_string(sql, None).unwrap();
    println!("{:#?}", parsed.tree);
}