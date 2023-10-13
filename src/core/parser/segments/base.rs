use crate::core::parser::markers::PositionMarker;
use dyn_clone::DynClone;
use std::fmt::Debug;
use uuid::Uuid;

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

pub trait Segment: DynClone + Debug {
    fn get_raw(&self) -> Option<String> {
        panic!("Not implemented yet");
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

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        panic!("Not implemented yet");
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        panic!("Not implemented yet");
    }

    /// Return the length of the segment in characters.
    fn get_matched_length(&self) -> usize {
        match self.get_raw() {
            None => 0,
            Some(raw) => raw.len(),
        }
    }

    /// Are we able to have non-code at the start or end?
    fn get_can_start_end_non_code(&self) -> bool {
        false
    }

    /// Can we allow it to be empty? Usually used in combination with the can_start_end_non_code.
    fn get_allow_empty(&self) -> bool {
        false
    }

    /// get_file_path returns the file path of the segment if it is a file segment.
    fn get_file_path(&self) -> Option<String> {
        None
    }

    /// Iterate raw segments, mostly for searching.
    ///
    /// In sqlfluff only implemented for RawSegments and up
    fn get_raw_segments(&self) -> Option<Vec<Box<dyn Segment>>> {
        None
    }

    fn get_uuid(&self) -> Option<Uuid>;

    fn indent_val(&self) -> usize {
        panic!("Not implemented yet");
    }
}

dyn_clone::clone_trait_object!(Segment);

impl PartialEq for Box<dyn Segment> {
    fn eq(&self, other: &Self) -> bool {
        match (self.get_uuid(), other.get_uuid()) {
            (Some(uuid1), Some(uuid2)) => {
                if uuid1 == uuid2 {
                    return true;
                };
            }
            _ => (),
        };
        let pos_self = self.get_position_marker();
        let pos_other = other.get_position_marker();
        if let (Some(pos_self), Some(pos_other)) = (pos_self, pos_other) {
            self.get_type() == other.get_type()
                && self.get_raw() == other.get_raw()
                && pos_self == pos_other
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeSegment {
    raw: String,
    position_marker: PositionMarker,
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
        Box::new(CodeSegment {
            raw: raw.to_string(),
            position_marker: position_maker.clone(),
            code_type: args.code_type,
        })
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
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
        _raw: &str,
        _position_maker: &PositionMarker,
        _args: CommentSegmentNewArgs,
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }
}

// Segment containing a newline.
#[derive(Debug, Clone)]
pub struct NewlineSegment {}

#[derive(Debug, Clone)]
pub struct NewlineSegmentNewArgs {}

impl NewlineSegment {
    pub fn new(
        _raw: &str,
        _position_maker: &PositionMarker,
        _args: NewlineSegmentNewArgs,
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }
}

/// Segment containing whitespace.
#[derive(Debug, Clone)]
pub struct WhitespaceSegment {}

#[derive(Debug, Clone)]
pub struct WhitespaceSegmentNewArgs;

impl WhitespaceSegment {
    pub fn new(
        _raw: &str,
        _position_maker: &PositionMarker,
        _args: WhitespaceSegmentNewArgs,
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }
}

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
        _raw: &str,
        _position_maker: &PositionMarker,
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
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

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct SymbolSegmentNewArgs {}

impl SymbolSegment {
    pub fn new(
        _raw: &str,
        _position_maker: &PositionMarker,
        _args: SymbolSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(SymbolSegment {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::markers::PositionMarker;
    use crate::core::parser::segments::raw::RawSegment;
    use crate::core::parser::segments::test_functions::raw_seg;
    use crate::core::templaters::base::TemplatedFile;

    #[test]
    /// Test comparison of raw segments.
    fn test__parser__base_segments_raw_compare() {
        let template = TemplatedFile::from_string("foobar".to_string());
        let rs1 = Box::new(RawSegment::new(
            Some("foobar".to_string()),
            Some(PositionMarker::new(
                0..6,
                0..6,
                template.clone(),
                None,
                None,
            )),
            None,
            None,
            None,
            None,
            None,
            None,
        )) as Box<dyn Segment>;
        let rs2 = Box::new(RawSegment::new(
            Some("foobar".to_string()),
            Some(PositionMarker::new(
                0..6,
                0..6,
                template.clone(),
                None,
                None,
            )),
            None,
            None,
            None,
            None,
            None,
            None,
        )) as Box<dyn Segment>;

        assert!(rs1 == rs2)
    }

    #[test]
    // TODO Implement
    /// Test raw segments behave as expected.
    fn test__parser__base_segments_raw() {
        let _raw_seg = raw_seg();
    }

    #[test]
    /// Test the .is_type() method.
    fn test__parser__base_segments_type() {
        let args = UnlexableSegmentNewArgs { expected: None };
        let segment = UnlexableSegment::new("", &PositionMarker::default(), args);

        assert!(segment.is_type("unlexable"));
        assert!(!segment.is_type("whitespace"));
    }
}
