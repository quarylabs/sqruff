use crate::dialect::Dialect;

pub struct AnsiDialect {}

impl Dialect for AnsiDialect {
    fn is_identifer_start(&self, ch: char) -> bool {
        (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')
    }
}
