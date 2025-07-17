use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use std::fs;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let tables = Tables::default();
    
    // Process select.sql
    let sql_path = "crates/lib-dialects/test/fixtures/dialects/tsql/select.sql";
    let yml_path = "crates/lib-dialects/test/fixtures/dialects/tsql/select.yml";
    
    if let Ok(sql) = fs::read_to_string(sql_path) {
        let lexer = Lexer::from(&dialect);
        let parser = Parser::from(&dialect);
        let tokens = lexer.lex(&tables, &sql);
        
        if tokens.1.is_empty() {
            if let Ok(parsed) = parser.parse(&tables, &tokens.0, None) {
                if let Some(tree) = parsed {
                    let tree = tree.to_serialised(true, true);
                    let yaml = serde_yaml::to_string(&tree).unwrap();
                    
                    println!("Writing {}", yml_path);
                    fs::write(yml_path, yaml).unwrap();
                }
            }
        }
    }
}