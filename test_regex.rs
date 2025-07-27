fn main() {
    let pattern = r"##?[a-zA-Z0-9_]+|[0-9a-zA-Z_]+#?";
    let re = regex::Regex::new(pattern).unwrap();
    
    let tests = vec!["CASE", "#temp", "##global", "col#", "col1", "123"];
    
    for test in tests {
        if let Some(m) = re.find(test) {
            println!("{}: matches '{}' at {:?}", test, m.as_str(), m.range());
        } else {
            println!("{}: no match", test);
        }
    }
}