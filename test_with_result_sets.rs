use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_dialects::kind::SyntaxKind;

fn main() {
    let sql = "EXECUTE test WITH RESULT SETS ((col1 INT));";
    
    let parser = Parser::new(sqruff_lib_dialects::tsql(), SyntaxKind::File);
    let parsed = parser.parse(sql, None);
    
    if let Some(tree) = parsed.tree {
        println!("Parse tree:\n{}", tree.to_tree_string());
        
        // Find unparsable sections
        for segment in tree.recursive_crawl(&SyntaxKind::UnparsableSegment.into(), true, &None, true) {
            let start = segment.range().start;
            let end = segment.range().end;
            let content = &sql[start..end];
            println!("\nUnparsable section at position {}: '{}'", start, content);
        }
    } else {
        println!("Failed to parse!");
    }
}