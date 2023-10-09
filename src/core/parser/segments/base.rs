use crate::core::parser::markers::PositionMarker;
use crate::core::rules::base::LintFix;
use dyn_clone::DynClone;
use std::fmt::Debug;
use std::hash::Hash;

/// An element of the response to BaseSegment.path_to().
///     Attributes:
///         segment (:obj:`BaseSegment`): The segment in the chain.
///         idx (int): The index of the target within its `segment`.
///         len (int): The number of children `segment` has.
#[derive(Debug, Clone)]
pub struct PathStep<S: Segment> {
    pub segment: S,
    pub idx: usize,
    pub len: usize,
}

pub type SegmentConstructorFn<SegmentArgs> =
    &'static dyn Fn(&str, &PositionMarker, SegmentArgs) -> Box<dyn Segment>;

pub trait Segment: DynClone {
    fn get_raw(&self) -> Option<&str> {
        None
    }
    fn get_type(&self) -> &'static str;
    fn is_type(&self, type_: &str) -> bool {
        self.get_type() == type_
    }
    fn is_code(&self) -> bool;
    fn is_comment(&self) -> bool;
    fn is_whitespace(&self) -> bool;
    fn is_meta(&self) -> bool {
        false
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        None
    }
    fn get_pos_maker(&self) -> Option<PositionMarker> {
        None
    }

    /// Return the length of the segment in characters.
    fn get_matched_length(&self) -> usize {
        match self.get_raw() {
            None => 0,
            Some(raw) => raw.len(),
        }
    }
    fn indent_val(&self) -> usize {
        panic!("Not implemented yet");
    }
}

dyn_clone::clone_trait_object!(Segment);

#[derive(Debug, Clone)]
pub struct CodeSegment {
    pub code_type: &'static str,
}

#[derive(Debug, Clone)]
pub struct CodeSegmentNewArgs {
    pub code_type: &'static str,
}

impl CodeSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: CodeSegmentNewArgs,
    ) -> Box<dyn Segment> {
        panic!("Not implemented yet")
    }
}

impl Segment for CodeSegment {
    fn get_type(&self) -> &'static str {
        "code"
    }
    fn is_code(&self) -> bool {
        true
    }
    fn is_comment(&self) -> bool {
        false
    }
    fn is_whitespace(&self) -> bool {
        false
    }
}

/// Segment containing a comment.
#[derive(Debug, Clone)]
pub struct CommentSegment {
    r#type: &'static str,
    trim_start: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct CommentSegmentNewArgs {
    pub r#type: &'static str,
    pub trim_start: Option<Vec<&'static str>>,
}

impl CommentSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: CommentSegmentNewArgs,
    ) -> Box<dyn Segment> {
        panic!("Not implemented yet")
    }
}

impl Segment for CommentSegment {
    fn get_type(&self) -> &'static str {
        "comment"
    }
    fn is_code(&self) -> bool {
        false
    }
    fn is_comment(&self) -> bool {
        true
    }
    fn is_whitespace(&self) -> bool {
        false
    }
}

// Segment containing a newline.
#[derive(Debug, Clone)]
pub struct NewlineSegment {}

#[derive(Debug, Clone)]
pub struct NewlineSegmentNewArgs {}

impl NewlineSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: NewlineSegmentNewArgs,
    ) -> Box<dyn Segment> {
        panic!("Not implemented yet")
    }
}

impl Segment for NewlineSegment {
    fn get_type(&self) -> &'static str {
        "newline"
    }
    fn is_code(&self) -> bool {
        false
    }
    fn is_comment(&self) -> bool {
        false
    }
    fn is_whitespace(&self) -> bool {
        true
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        Some("\n")
    }
}

/// Segment containing whitespace.
#[derive(Debug, Clone)]
pub struct WhitespaceSegment {}

#[derive(Debug, Clone)]
pub struct WhitespaceSegmentNewArgs;

impl WhitespaceSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: WhitespaceSegmentNewArgs,
    ) -> Box<dyn Segment> {
        panic!("Not implemented yet")
    }
}

impl Segment for WhitespaceSegment {
    fn get_type(&self) -> &'static str {
        "whitespace"
    }
    fn is_code(&self) -> bool {
        false
    }
    fn is_comment(&self) -> bool {
        false
    }
    fn is_whitespace(&self) -> bool {
        true
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        Some(" ")
    }
}

// /// A placeholder to un-lexable sections.
// ///
// /// This otherwise behaves exactly like a code section.
// #[derive(Debug, Clone)]
// pub struct RawSegment {}
//
// #[derive(Debug, Clone)]
// pub struct RawSegmentNewArgs;
//
// impl Segment<RawSegmentNewArgs> for RawSegment {
//     fn new(raw: &str, position_maker: PositionMarker, args: RawSegmentNewArgs) -> Self {
//         panic!("Not implemented yet")
//     }
//     fn get_type(&self) -> &'static str {
//         "raw"
//     }
//     fn is_code(&self) -> bool {
//         true
//     }
//     fn is_comment(&self) -> bool {
//         false
//     }
//     fn is_whitespace(&self) -> bool {
//         false
//     }
// }
//
// /// A segment used for matching single entities which aren't keywords.
// ///
// /// We rename the segment class here so that descendants of
// /// _ProtoKeywordSegment can use the same functionality
// /// but don't end up being labelled as a `keyword` later.
// #[derive(Debug, Clone)]
// pub struct SymbolSegment {}
//
// #[derive(Debug, Clone)]
// pub struct SymbolSegmentNewArgs;
//
// impl Segment<SymbolSegmentNewArgs> for SymbolSegment {
//     fn new(raw: &str, position_maker: PositionMarker, args: SymbolSegmentNewArgs) -> Self {
//         panic!("Not implemented yet")
//     }
//     fn get_type(&self) -> &'static str {
//         "symbol"
//     }
//     fn is_code(&self) -> bool {
//         true
//     }
//     fn is_comment(&self) -> bool {
//         false
//     }
//     fn is_whitespace(&self) -> bool {
//         false
//     }
// }
//
#[derive(Debug, Clone)]
pub struct UnlexableSegment {
    expected: String,
}

#[derive(Debug, Clone)]
pub struct UnlexableSegmentNewArgs {
    pub(crate) expected: Option<String>,
}

impl UnlexableSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: UnlexableSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(UnlexableSegment {
            expected: args.expected.unwrap_or("".to_string()),
        })
    }
}

impl Segment for UnlexableSegment {
    fn get_type(&self) -> &'static str {
        "unlexable"
    }
    fn is_code(&self) -> bool {
        true
    }
    fn is_comment(&self) -> bool {
        false
    }
    fn is_whitespace(&self) -> bool {
        false
    }
}

/// A segment used for matching single entities which aren't keywords.
///
///     We rename the segment class here so that descendants of
///     _ProtoKeywordSegment can use the same functionality
///     but don't end up being labelled as a `keyword` later.
#[derive(Debug, Clone)]
pub struct SymbolSegment {}

impl Segment for SymbolSegment {
    fn get_type(&self) -> &'static str {
        return "symbol";
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct SymbolSegmentNewArgs {}

impl SymbolSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: SymbolSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(SymbolSegment {})
    }
}

mod tests {
    use super::*;
    use crate::core::parser::markers::PositionMarker;

    #[test]
    /// Test the .is_type() method.
    fn test__parser__base_segments_type() {
        let args = UnlexableSegmentNewArgs { expected: None };
        let segment = UnlexableSegment::new("", &PositionMarker::default(), args);

        assert!(segment.is_type("unlexable"));
        assert!(!segment.is_type("whitespace"));
    }
}
