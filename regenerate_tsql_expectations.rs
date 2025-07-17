use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use std::fs;
use std::path::Path;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let tables = Tables::default();
    
    // List of test files to regenerate
    let test_files = vec![
        "select",
        "begin_end_block", 
        "temporal_tables",
        "create_table_as_select",
    ];
    
    for file_name in test_files {
        let sql_path = format!("crates/lib-dialects/test/fixtures/dialects/tsql/{}.sql", file_name);
        let yml_path = format!("crates/lib-dialects/test/fixtures/dialects/tsql/{}.yml", file_name);
        
        println!("Processing {}...", file_name);
        
        if let Ok(sql) = fs::read_to_string(&sql_path) {
            let lexer = Lexer::from(&dialect);
            let parser = Parser::from(&dialect);
            let (tokens, errors) = lexer.lex(&tables, &sql);
            
            if !errors.is_empty() {
                println!("  Lexer errors in {}: {:?}", file_name, errors);
                continue;
            }
            
            match parser.parse(&tables, &tokens, None) {
                Ok(Some(tree)) => {
                    let serialized = tree.to_serialised(true, true);
                    let yaml = serde_yaml::to_string(&serialized).unwrap();
                    
                    println!("  Writing {}", yml_path);
                    fs::write(&yml_path, yaml).unwrap();
                }
                Ok(None) => println!("  Parser returned None for {}", file_name),
                Err(e) => println!("  Parse error in {}: {:?}", file_name, e),
            }
        } else {
            println!("  Could not read {}", sql_path);
        }
    }
}