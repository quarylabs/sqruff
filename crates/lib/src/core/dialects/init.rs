use super::base::Dialect;

pub fn dialect_selector(s: &str) -> Option<Dialect> {
    match s {
        "ansi" => Some(crate::dialects::ansi::ansi_dialect()),
        _ => None,
    }
}

pub fn get_default_dialect() -> &'static str {
    return "ansi";
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
