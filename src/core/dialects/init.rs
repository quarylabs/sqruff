use crate::core::dialects::base::Dialect;

pub fn dialect_selector(s: &String) -> Option<Box<dyn Dialect>> {
    match s.as_str() {
        "ansi" => Some(Box::new(crate::dialects::ansi::AnsiDialect {})),
        _ => None,
    }
}

/// Dialect Tuple object for describing dialects.
pub struct DialectTuple {
    pub label: String,
    pub name: String,
    pub inherits_from: String,
}

/// Generate a readout of available dialects.
pub fn dialect_readout<'a>() -> Vec<String> {
    panic!("dialect_readout not implemented yet");
}
