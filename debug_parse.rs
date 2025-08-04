use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_dialects::init::DialectKind;
use std::env;

fn print_tree(segment: &sqruff_lib_core::parser::segments::ErasedSegment, indent: usize) {
    let indent_str = " ".repeat(indent);
    println!("{}{:?} - raw: {:?}", indent_str, segment.get_type(), segment.raw());
    for child in segment.segments() {
        print_tree(child, indent + 2);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let dialect = args.get(1).unwrap_or(&"ansi".to_string()).clone();
    let sql = args.get(2).unwrap_or(&"SELECT 1".to_string()).clone();
    
    println!("Parsing SQL: {}", sql);
    println!("Dialect: {}", dialect);
    println!();
    
    let dialect_kind = match dialect.as_str() {
        "tsql" => DialectKind::Tsql,
        "ansi" => DialectKind::Ansi,
        _ => panic!("Unknown dialect: {}", dialect),
    };
    
    let mut parser = Parser::new(dialect_kind);
    let parsed = parser.parse(&sql, None);
    
    if let Ok(tree) = parsed {
        print_tree(&tree, 0);
    } else {
        println!("Parse failed: {:?}", parsed);
    }
}