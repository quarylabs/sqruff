use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::fix::{AnchorEditInfo, FixPatch, SourceFix};
use crate::core::rules::base::LintFix;
use crate::core::templaters::base::TemplatedFile;
use dyn_clone::DynClone;
use std::collections::HashMap;
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
    fn get_raw(&self) -> Option<String>;
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
    fn get_position_marker(&self) -> Option<PositionMarker>;
    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>);

    // get_segments is the way the segment returns its children 'self.segments' in Python.
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        todo!()
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

    /// Yield any source patches as fixes now.
    ///
    ///         NOTE: This yields source fixes for the segment and any of its
    ///         children, so it's important to call it at the right point in
    ///         the recursion to avoid yielding duplicates.
    fn iter_source_fix_patches(&self, templated_file: &TemplatedFile) -> Vec<FixPatch> {
        let mut patches = Vec::new();
        for source_fix in &self.get_source_fixes() {
            patches.push(FixPatch::new(
                source_fix.templated_slice.clone(),
                source_fix.edit.clone(),
                String::from("source"),
                source_fix.source_slice.clone(),
                templated_file.templated_str.clone().unwrap()[source_fix.templated_slice.clone()]
                    .to_string(),
                templated_file.source_str[source_fix.source_slice.clone()].to_string(),
            ));
        }
        patches
    }

    fn get_uuid(&self) -> Option<Uuid>;

    fn indent_val(&self) -> usize {
        panic!("Not implemented yet");
    }

    /// Return any source fixes as list.
    fn get_source_fixes(&self) -> Vec<SourceFix> {
        self.get_raw_segments()
            .unwrap_or(vec![])
            .iter()
            .flat_map(|seg| seg.get_source_fixes())
            .collect()
    }

    /// Stub.
    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> Box<dyn Segment>;

    /// Group and count fixes by anchor, return dictionary.
    fn compute_anchor_edit_info(&self, fixes: &Vec<LintFix>) -> HashMap<Uuid, AnchorEditInfo> {
        let mut anchor_info = HashMap::<Uuid, AnchorEditInfo>::new();
        for fix in fixes {
            // :TRICKY: Use segment uuid as the dictionary key since
            // different segments may compare as equal.
            let anchor_id = fix.anchor.get_uuid().unwrap();
            anchor_info.entry(anchor_id).or_default().add(fix.clone());
        }
        anchor_info
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
    position_marker: Option<PositionMarker>,
    pub code_type: &'static str,

    // From RawSegment
    uuid: Uuid,
    instance_types: Vec<String>,
    trim_start: Option<Vec<String>>,
    trim_chars: Option<Vec<String>>,
    source_fixes: Option<Vec<SourceFix>>,
}

#[derive(Debug, Clone)]
pub struct CodeSegmentNewArgs {
    pub code_type: &'static str,

    pub instance_types: Vec<String>,
    pub trim_start: Option<Vec<String>>,
    pub trim_chars: Option<Vec<String>>,
    pub source_fixes: Option<Vec<SourceFix>>,
}

impl CodeSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: CodeSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(CodeSegment {
            raw: raw.to_string(),
            position_marker: Some(position_maker.clone()),
            code_type: args.code_type,

            instance_types: vec![],
            trim_start: None,
            trim_chars: None,
            source_fixes: None,
            uuid: uuid::Uuid::new_v4(),
        })
    }
}

impl Segment for CodeSegment {
    fn get_raw(&self) -> Option<String> {
        Some(self.raw.clone())
    }
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker
    }

    fn get_uuid(&self) -> Option<Uuid> {
        Some(self.uuid.clone())
    }

    /// Create a new segment, with exactly the same position but different content.
    ///
    ///         Returns:
    ///             A copy of this object with new contents.
    ///
    ///         Used mostly by fixes.
    ///
    ///         NOTE: This *doesn't* copy the uuid. The edited segment is a new segment.
    ///
    /// From RawSegment implementation
    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> Box<dyn Segment> {
        CodeSegment::new(
            raw.unwrap_or(self.raw.clone()).as_str(),
            &self.position_marker.clone().unwrap(),
            CodeSegmentNewArgs {
                code_type: self.code_type,
                instance_types: vec![],
                trim_start: None,
                source_fixes,
                trim_chars: None,
            },
        )
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }

    fn get_raw(&self) -> Option<String> {
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn get_raw(&self) -> Option<String> {
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
    fn get_raw(&self) -> Option<String> {
        todo!()
    }

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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
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
    fn get_raw(&self) -> Option<String> {
        todo!()
    }
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
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
    fn get_raw(&self) -> Option<String> {
        todo!()
    }

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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
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
    use crate::core::parser::segments::test_functions::{raw_seg, raw_segments};
    use crate::core::rules::base::LintFix;
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
    /// Test BaseSegment.compute_anchor_edit_info().
    fn test__parser_base_segments_compute_anchor_edit_info() {
        let raw_segs = raw_segments();

        // Construct a fix buffer, intentionally with:
        // - one duplicate.
        // - two different incompatible fixes on the same segment.
        let fixes = vec![
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("a".to_string()), None)],
                None,
            ),
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("a".to_string()), None)],
                None,
            ),
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("b".to_string()), None)],
                None,
            ),
        ];

        let anchor_edit_info = raw_segs[0].compute_anchor_edit_info(&fixes);

        // Check the target segment is the only key we have.
        assert_eq!(
            anchor_edit_info.keys().collect::<Vec<_>>(),
            vec![&raw_segs[0].get_uuid().unwrap()]
        );

        let anchor_info = anchor_edit_info
            .get(&raw_segs[0].get_uuid().unwrap())
            .unwrap();

        // Check that the duplicate as been deduplicated i.e. this isn't 3.
        assert_eq!(anchor_info.replace, 2);

        // Check the fixes themselves.
        //   Note: There's no duplicated first fix.
        assert_eq!(
            anchor_info.fixes[0],
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("a".to_string()), None)],
                None,
            )
        );
        assert_eq!(
            anchor_info.fixes[1],
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("b".to_string()), None)],
                None,
            )
        );

        // Check the first replace
        assert_eq!(
            anchor_info.first_replace_fix,
            Some(LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(Some("a".to_string()), None)],
                None,
            ))
        );
    }

    /// Test the .is_type() method.
    #[test]
    fn test__parser__base_segments_type() {
        let args = UnlexableSegmentNewArgs { expected: None };
        let segment = UnlexableSegment::new("", &PositionMarker::default(), args);

        assert!(segment.is_type("unlexable"));
        assert!(!segment.is_type("whitespace"));
    }
}
