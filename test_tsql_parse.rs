use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let parser = Parser::new(&dialect);
    
    let sql = r#"
SELECT 
    CASE 
        WHEN status = 'A' THEN 'Active'
        WHEN status = 'I' THEN 'Inactive'
        ELSE 'Unknown'
    END AS status_desc
FROM users;
"#;
    
    let result = parser.parse_string(sql);
    match result {
        Ok(parsed) => {
            println!("Parse successful!");
            println!("AST: {:#?}", parsed.tree);
            
            // Check for unparsable segments
            let unparsable_segments = parsed.tree.find_all("unparsable");
            if unparsable_segments.is_empty() {
                println!("\n✅ No unparsable segments found!");
            } else {
                println!("\n❌ Found {} unparsable segments", unparsable_segments.len());
                for (i, seg) in unparsable_segments.iter().enumerate() {
                    println!("  Unparsable segment {}: {:?}", i + 1, seg);
                }
            }
        }
        Err(e) => {
            println!("Parse failed: {:?}", e);
        }
    }
}