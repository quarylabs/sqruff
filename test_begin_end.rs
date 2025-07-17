use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;

fn main() {
    let dialect = kind_to_dialect(&DialectKind::Tsql).unwrap();
    let tables = Tables::default();
    
    let sql = "BEGIN SELECT * FROM customers; UPDATE customers SET status = 'Active'; END";
    
    let lexer = Lexer::from(&dialect);
    let parser = Parser::from(&dialect);
    let (tokens, errors) = lexer.lex(&tables, sql);
    
    if !errors.is_empty() {
        println!("Lexer errors: {:?}", errors);
    }
    
    println!("Tokens: {:?}", tokens.iter().map(|t| t.raw()).collect::<Vec<_>>());
    
    match parser.parse(&tables, &tokens, None) {
        Ok(Some(tree)) => {
            println!("Parse successful!");
            let serialized = tree.to_serialised(true, true);
            println!("{}", serde_yaml::to_string(&serialized).unwrap());
        }
        Ok(None) => println!("Parser returned None"),
        Err(e) => println!("Parse error: {:?}", e),
    }
}