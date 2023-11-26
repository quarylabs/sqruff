use crate::core::{
    config::FluffConfig,
    dialects::{base::Dialect, init::dialect_selector},
};

#[derive(Debug)]
pub struct ParseContext {
    dialect: Dialect,
    // recurse: bool,
    // indentation_config: HashMap<String, bool>,
    // denylist: ParseDenylist,
    // logger: Logger,
    // uuid: uuid::Uuid,
}

impl ParseContext {
    pub fn new(dialect: Dialect) -> Self {
        Self { dialect }
    }

    pub fn from_config(_config: FluffConfig) -> Self {
        let dialect = dialect_selector("ansi").unwrap();
        Self::new(dialect)
    }
}
