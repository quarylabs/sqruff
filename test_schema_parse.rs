use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::Linter;

fn main() {
    let sql = "SELECT * FROM dbo.target;";
    let config = FluffConfig::from_source(
        r#"
[sqruff]
dialect = tsql
"#,
    )
    .unwrap();
    
    let linter = Linter::new(config, None, None);
    let result = linter.lint_string_with_tree(sql, None, false);
    
    match result {
        Ok((tree, violations)) => {
            println!("Tree parsed successfully!");
            println!("Tree: {:#?}", tree);
            println!("Violations: {:?}", violations);
        }
        Err(e) => {
            println!("Failed to parse: {:?}", e);
        }
    }
}