use sqruff_lib_core::dialects::DialectKind;
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib::Config;

fn main() {
    // Test SQL with CASE expression
    let sql = "SELECT CASE WHEN 1=1 THEN 'A' END;";
    
    // Create config with T-SQL dialect
    let mut config = Config::default();
    config.dialect = DialectKind::Tsql;
    
    // Parse the SQL
    let parser = Parser::new(&config, None, true);
    let tables = parser.parse(sql, None, None, None).unwrap();
    let parsed = tables.tree().unwrap();
    
    // Print the parsed tree
    println!("Parsed tree:");
    println!("{:#?}", parsed);
    
    // Look for unparsable segments
    fn find_unparsable(node: &sqruff_lib_core::parser::segments::ErasedSegment, depth: usize) {
        let indent = "  ".repeat(depth);
        if node.raw().is_empty() {
            return;
        }
        
        let syntax_kind = node.get_syntax_kind();
        let raw = node.raw();
        
        println!("{}{:?}: '{}'", indent, syntax_kind, raw);
        
        if syntax_kind == sqruff_lib_core::dialects::syntax::SyntaxKind::Unparsable {
            println!("{}  ^ UNPARSABLE!", indent);
        }
        
        for child in node.segments() {
            find_unparsable(child, depth + 1);
        }
    }
    
    println!("\nTraversing tree to find issues:");
    find_unparsable(&parsed, 0);
}