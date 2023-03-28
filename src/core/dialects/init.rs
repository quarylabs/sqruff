use crate::core::dialects::base::Dialect;

pub fn dialect_selector(s: &String) -> Option<Box<dyn Dialect>> {
    match s.as_str() {
        "ansi" => Some(Box::new(crate::dialects::ansi::AnsiDialect {})),
        _ => None,
    }    
}
