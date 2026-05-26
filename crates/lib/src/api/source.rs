use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceId {
    Stdin,
    Path(PathBuf),
    Virtual(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source<'a> {
    pub id: SourceId,
    pub text: Cow<'a, str>,
}
