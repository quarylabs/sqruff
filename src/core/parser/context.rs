use crate::{
    core::{config::FluffConfig, dialects::base::Dialect},
    dialects::ansi::AnsiDialect,
};

#[derive(Debug)]
pub struct ParseContext {
    dialect: Box<dyn Dialect>,
    // recurse: bool,
    // indentation_config: HashMap<String, bool>,
    // denylist: ParseDenylist,
    // logger: Logger,
    // uuid: uuid::Uuid,
}

impl ParseContext {
    pub fn new(dialect: Box<dyn Dialect>) -> Self {
        Self { dialect }
    }

    pub fn from_config(_config: FluffConfig) -> Self {
        let dialect = Box::new(AnsiDialect);
        Self::new(dialect)
    }
}
