use super::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Fix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrors {
    Suppress,
    Include,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineOptions {
    pub parse_errors: ParseErrors,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            parse_errors: ParseErrors::Suppress,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest<'a> {
    pub mode: Mode,
    pub sources: Vec<Source<'a>>,
}
