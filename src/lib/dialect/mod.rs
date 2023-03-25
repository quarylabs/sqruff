pub mod ansi;
pub mod generic;
pub mod mysql;
pub mod postgresql;

pub trait Dialect {
    // Determine if a character is a valid start character for an unquoted identifier
    fn is_identifer_start(&self, ch: char) -> bool;
}
