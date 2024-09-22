use super::matchable::Matchable;
use super::segments::generator::SegmentGenerator;

#[derive(Debug, Clone)]
pub enum DialectElementType {
    Matchable(Matchable),
    SegmentGenerator(SegmentGenerator),
}

impl From<Matchable> for DialectElementType {
    fn from(value: Matchable) -> Self {
        DialectElementType::Matchable(value)
    }
}

impl From<SegmentGenerator> for DialectElementType {
    fn from(value: SegmentGenerator) -> Self {
        DialectElementType::SegmentGenerator(value)
    }
}

/// ParseMode defines the potential parse modes used in grammars
/// to determine how they handle unmatched segments.
///
/// The default behavior is to only claim what they can match. However,
/// occasionally allowing more eager matching (e.g., in the content of
/// bracketed expressions) can provide more helpful feedback to the user.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ParseMode {
    /// Strict mode only returns a match if the full content matches.
    /// In this mode, if a match is not successful, then no match is returned,
    /// and unparsable sections are never raised.
    ///
    /// Note: This is the default for all grammars.
    Strict,

    /// Greedy mode will always return a match, provided there is at least
    /// one code element before any terminators. Terminators are not included
    /// in the match but are searched for before matching any content. Segments
    /// that are part of any terminator (or beyond) are not available for
    /// matching by any content.
    ///
    /// Note: This replicates the `GreedyUntil` semantics.
    Greedy,

    /// A variant of "GREEDY" mode. This mode behaves like "STRICT" if nothing
    /// matches, but behaves like "GREEDY" once something has matched.
    ///
    /// Note: This replicates the `StartsWith` semantics.
    GreedyOnceStarted,
    // Additional parsing modes can be added here as necessary.
}
