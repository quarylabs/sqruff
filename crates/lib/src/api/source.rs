use std::borrow::Cow;
use std::path::PathBuf;

use crate::api::SkipReason;
use crate::core::linter::common::ParsedString;

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

/// Pre-parsed SQL source that can be used with [`Engine::fix_parsed`] and
/// [`Engine::check_parsed`] to avoid re-parsing on every call.
#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub(crate) source_id: SourceId,
    pub(crate) parsed: ParsedString,
    pub(crate) skip_reason: Option<SkipReason>,
}

