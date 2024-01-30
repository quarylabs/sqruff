use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem::take;

use dyn_clone::DynClone;
use dyn_hash::DynHash;
use dyn_ord::DynEq;
use itertools::Itertools;
use uuid::Uuid;

use crate::core::dialects::base::Dialect;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::fix::{AnchorEditInfo, FixPatch, SourceFix};
use crate::core::rules::base::{EditType, LintFix};
use crate::core::templaters::base::TemplatedFile;
use crate::helpers::Boxed;

/// An element of the response to BaseSegment.path_to().
///     Attributes:
///         segment (:obj:`BaseSegment`): The segment in the chain.
///         idx (int): The index of the target within its `segment`.
///         len (int): The number of children `segment` has.
#[derive(Debug, Clone)]
pub struct PathStep {
    pub segment: Box<dyn Segment>,
    pub idx: usize,
    pub len: usize,
    pub code_idxs: Vec<usize>,
}

pub type SegmentConstructorFn<SegmentArgs> =
    &'static dyn Fn(&str, &PositionMarker, SegmentArgs) -> Box<dyn Segment>;

pub trait CloneSegment {
    fn clone_box(&self) -> Box<dyn Segment>;
}

impl<T: Segment + DynClone> CloneSegment for T {
    fn clone_box(&self) -> Box<dyn Segment> {
        dyn_clone::clone(self).boxed()
    }
}

pub trait Segment: Any + DynEq + DynClone + DynHash + Debug + CloneSegment {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn print_tree(&self) {
        let mut tree = String::new();

        let mut display = |seg: &dyn Segment| {
            tree.push_str(&format!("{} = {}", seg.get_type(), seg.get_raw().unwrap()))
        };

        display(&*self.clone_box());
        for seg in self.get_segments() {
            seg.print_tree();
        }

        println!("{tree}");
    }

    fn code_indices(&self) -> Vec<usize> {
        self.get_segments()
            .iter()
            .enumerate()
            .filter(|(_, seg)| seg.is_code())
            .map(|(idx, _)| idx)
            .collect()
    }

    fn get_parent(&self) -> Option<Box<dyn Segment>> {
        None
    }

    fn child(&self, seg_types: &[&str]) -> Option<Box<dyn Segment>> {
        for seg in self.get_segments() {
            if seg_types.iter().any(|ty| seg.is_type(ty)) {
                return Some(seg);
            }
        }
        None
    }

    fn children(&self, seg_types: &[&str]) -> Vec<Box<dyn Segment>> {
        let mut buff = Vec::new();
        for seg in self.get_segments() {
            if seg_types.iter().any(|ty| seg.is_type(ty)) {
                buff.push(seg);
            }
        }
        buff
    }

    fn path_to(&self, other: &Box<dyn Segment>) -> Vec<PathStep> {
        if self.dyn_eq(other) {
            return Vec::new();
        }

        if self.get_segments().is_empty() {
            return Vec::new();
        }

        let mut midpoint = other.clone();
        let mut lower_path = Vec::new();

        while let Some(higher) = midpoint.get_parent() {
            assert!(
                higher.get_position_marker().is_some(),
                "`path_to()` found segment {higher:?} without position. This shouldn't happen \
                 post-parse."
            );
            lower_path.push(PathStep {
                segment: higher.clone(),
                idx: higher.get_segments().iter().position(|seg| seg.dyn_eq(&*higher)).unwrap(),
                len: higher.get_segments().len(),
                code_idxs: higher.code_indices(),
            });
            midpoint = higher.clone();
            if self.dyn_eq(&midpoint) {
                break;
            }
        }

        lower_path.reverse();

        if self.dyn_eq(&midpoint) {
            return lower_path;
        } else if midpoint.is_type("file") {
            return Vec::new();
        }

        // else if !(self.get_start_loc() <= midpoint.get_start_loc()
        //     && midpoint.get_start_loc() <= self.get_end_loc())
        // {
        //     return Vec::new();
        // }

        for (idx, seg) in self.get_segments().iter().enumerate() {
            // seg.set_parent(self); // Requires mutable reference to self or change in
            // design

            let step = PathStep {
                segment: self.clone_box(),
                idx,
                len: self.get_segments().len(),
                code_idxs: self.code_indices(),
            };
            if seg.dyn_eq(&midpoint) {
                let mut result = vec![step];
                result.extend(lower_path);
                return result;
            }
            let res = seg.path_to(&midpoint);
            if !res.is_empty() {
                let mut result = vec![step];
                result.extend(res);
                result.extend(lower_path);

                return result;
            }
        }

        Vec::new()
    }

    fn iter_patches(&self, templated_file: &TemplatedFile) -> Vec<FixPatch> {
        let mut acc = Vec::new();

        if self.get_position_marker().unwrap().is_literal() {
            acc.extend(self.iter_source_fix_patches(templated_file));
            acc.push(FixPatch::new(
                self.get_position_marker().unwrap().templated_slice,
                self.get_raw().unwrap(),
                "literal".into(),
                self.get_position_marker().unwrap().source_slice,
                templated_file.templated_str.as_ref().unwrap()
                    [self.get_position_marker().unwrap().templated_slice]
                    .to_string(),
                templated_file.source_str[self.get_position_marker().unwrap().source_slice]
                    .to_string(),
            ));
        }

        acc
    }

    fn descendant_type_set(&self) -> HashSet<String> {
        let mut result_set = HashSet::new();

        for seg in &self.get_segments() {
            // Combine descendant_type_set and class_types of each segment
            result_set.extend(seg.descendant_type_set().union(&seg.class_types()).cloned());
        }

        result_set
    }

    // TODO: remove &self?
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        None
    }

    fn get_raw(&self) -> Option<String> {
        self.get_segments().iter().filter_map(|segment| segment.get_raw()).join("").into()
    }

    fn get_raw_upper(&self) -> Option<String> {
        self.get_raw()?.to_uppercase().into()
    }

    // Assuming `raw_segments` is a field that holds a collection of segments
    fn first_non_whitespace_segment_raw_upper(&self) -> Option<String> {
        for seg in self.get_raw_segments() {
            // Assuming `raw_upper` is a method or field that returns a String
            if !seg.get_raw_upper().unwrap().trim().is_empty() {
                // Return Some(String) if the condition is met
                return Some(seg.get_raw_upper().unwrap());
            }
        }
        // Return None if no non-whitespace segment is found
        None
    }

    fn get_type(&self) -> &'static str {
        std::any::type_name::<Self>().split("::").last().unwrap()
    }
    fn is_type(&self, type_: &str) -> bool {
        self.get_type() == type_
    }
    fn is_code(&self) -> bool {
        self.get_segments().iter().any(|s| s.is_code())
    }
    fn is_comment(&self) -> bool {
        unimplemented!("{}", std::any::type_name::<Self>())
    }
    fn is_whitespace(&self) -> bool {
        unimplemented!("{}", std::any::type_name::<Self>())
    }
    fn is_meta(&self) -> bool {
        false
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        None
    }

    #[track_caller]
    fn get_position_marker(&self) -> Option<PositionMarker> {
        let markers: Vec<_> =
            self.get_segments().into_iter().flat_map(|seg| seg.get_position_marker()).collect();

        let pos = PositionMarker::from_child_markers(markers);

        Some(pos)
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    // get_segments is the way the segment returns its children 'self.segments' in
    // Python.
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        unimplemented!("{}", std::any::type_name::<Self>())
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

    /// Can we allow it to be empty? Usually used in combination with the
    /// can_start_end_non_code.
    fn get_allow_empty(&self) -> bool {
        false
    }

    /// get_file_path returns the file path of the segment if it is a file
    /// segment.
    fn get_file_path(&self) -> Option<String> {
        None
    }

    /// Iterate raw segments, mostly for searching.
    ///
    /// In sqlfluff only implemented for RawSegments and up
    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        self.get_segments().into_iter().flat_map(|item| item.get_raw_segments()).collect_vec()
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

    fn get_uuid(&self) -> Option<Uuid> {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn indent_val(&self) -> usize {
        panic!("Not implemented yet");
    }

    /// Return any source fixes as list.
    fn get_source_fixes(&self) -> Vec<SourceFix> {
        self.get_segments().iter().flat_map(|seg| seg.get_source_fixes()).collect()
    }

    /// Stub.
    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> Box<dyn Segment> {
        unimplemented!()
    }

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

    fn instance_types(&self) -> HashSet<String> {
        HashSet::new()
    }

    fn combined_types(&self) -> HashSet<String> {
        let mut combined = self.instance_types();
        combined.extend(self.class_types());
        combined
    }

    fn class_types(&self) -> HashSet<String> {
        HashSet::new()
    }

    #[allow(unused_variables)]
    fn apply_fixes(
        &self,
        dialect: Dialect,
        mut fixes: HashMap<Uuid, AnchorEditInfo>,
    ) -> (Box<dyn Segment>, Vec<Box<dyn Segment>>, Vec<Box<dyn Segment>>, bool) {
        let mut seg_buffer = Vec::new();
        let mut fixes_applied = Vec::new();
        let mut requires_validate = false;

        for seg in self.get_segments() {
            // Look for uuid match.
            // This handles potential positioning ambiguity.

            let Some(anchor_info) = fixes.remove(&seg.get_uuid().unwrap()) else {
                seg_buffer.push(seg);
                continue;
            };

            for f in &anchor_info.fixes {
                // assert f.anchor.uuid == seg.uuid
                fixes_applied.push(f.clone());

                // Deletes are easy.
                if f.edit_type == EditType::Delete {
                    // We're just getting rid of this segment.
                    requires_validate = true;
                    // NOTE: We don't add the segment in this case.
                    continue;
                }

                if f.edit_type == EditType::CreateAfter && anchor_info.fixes.len() == 1 {
                    // In the case of a creation after that is not part
                    // of a create_before/create_after pair, also add
                    // this segment before the edit.
                    seg_buffer.push(seg.clone());
                }

                for s in f.edit.as_ref().unwrap() {
                    seg_buffer.push(s.clone());
                }
            }
        }

        let seg_queue = seg_buffer.clone();
        let mut seg_buffer = Vec::new();
        for seg in seg_queue {
            let (s, pre, post, validated) = seg.apply_fixes(dialect.clone(), fixes.clone());
            seg_buffer.extend(pre);
            seg_buffer.push(s);
            seg_buffer.extend(post);

            if !validated {
                requires_validate = true;
            }
        }

        (self.new(seg_buffer), Vec::new(), Vec::new(), false)
    }

    fn raw_segments_with_ancestors(&self) -> Vec<(Box<dyn Segment>, Vec<PathStep>)> {
        let mut buffer: Vec<(Box<dyn Segment>, Vec<PathStep>)> = Vec::new();
        let code_idxs: Vec<usize> = self.code_indices();

        for (idx, seg) in self.get_segments().iter().enumerate() {
            let mut new_step = vec![PathStep {
                segment: self.clone_box(),
                idx,
                len: self.get_segments().len(),
                code_idxs: code_idxs.clone(),
            }];

            // Use seg.get_segments().is_empty() as a workaround to check if the segment is
            // a "raw" type. In the original Python code, this was achieved
            // using seg.is_type("raw"). Here, we assume that a "raw" segment is
            // characterized by having no sub-segments.
            if seg.get_segments().is_empty() {
                buffer.push((seg.clone(), new_step));
            } else {
                let mut extended = seg
                    .raw_segments_with_ancestors()
                    .into_iter()
                    .map(|(raw_seg, stack)| {
                        let mut new_step = take(&mut new_step);
                        new_step.extend(stack);
                        (raw_seg, new_step)
                    })
                    .collect::<Vec<_>>();

                buffer.append(&mut extended);
            }
        }

        buffer
    }
}

dyn_clone::clone_trait_object!(Segment);
dyn_hash::hash_trait_object!(Segment);

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

#[derive(Hash, Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Default)]
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
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        self.clone().boxed()
    }

    fn class_types(&self) -> HashSet<String> {
        Some(self.get_type().to_owned()).into_iter().collect()
    }

    fn get_raw(&self) -> Option<String> {
        Some(self.raw.clone())
    }
    fn get_type(&self) -> &'static str {
        self.code_type
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
        self.uuid.into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![]
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
    }

    /// Create a new segment, with exactly the same position but different
    /// content.
    ///
    ///         Returns:
    ///             A copy of this object with new contents.
    ///
    ///         Used mostly by fixes.
    ///
    ///         NOTE: This *doesn't* copy the uuid. The edited segment is a new
    /// segment.
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
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct CommentSegment {
    raw: String,
    r#type: &'static str,
    trim_start: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentSegmentNewArgs {
    pub r#type: &'static str,
    pub trim_start: Option<Vec<&'static str>>,
}

impl CommentSegment {
    pub fn new(
        raw: &str,
        _position_maker: &PositionMarker,
        args: CommentSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Self {
            raw: raw.to_string(),
            r#type: args.r#type,
            trim_start: args.trim_start.unwrap_or_default(),
        }
        .boxed()
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

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
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
        self.raw.clone().into()
    }
}

// Segment containing a newline.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct NewlineSegment {
    raw: String,
    position_maker: PositionMarker,
    uuid: Uuid,
}

#[derive(Debug, Clone)]
pub struct NewlineSegmentNewArgs {}

impl NewlineSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        _args: NewlineSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(NewlineSegment {
            raw: raw.to_string(),
            position_maker: position_maker.clone(),
            uuid: Uuid::new_v4(),
        })
    }
}

impl Segment for NewlineSegment {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        self.clone().boxed()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone_box()]
    }

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
        self.position_maker.clone().into()
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
        self.uuid.into()
    }

    fn get_raw(&self) -> Option<String> {
        Some(self.raw.clone())
    }
}

/// Segment containing whitespace.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct WhitespaceSegment {
    raw: String,
    position_marker: PositionMarker,
    uuid: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceSegmentNewArgs;

impl WhitespaceSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        _args: WhitespaceSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(WhitespaceSegment {
            raw: raw.to_string(),
            position_marker: position_maker.clone(),
            uuid: Uuid::new_v4(),
        })
    }
}

impl Segment for WhitespaceSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self {
            raw: self.get_raw().unwrap(),
            position_marker: self.position_marker.clone(),
            uuid: self.uuid.clone(),
        }
        .boxed()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
    }

    fn get_raw(&self) -> Option<String> {
        Some(self.raw.clone())
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
        self.position_marker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn edit(&self, raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> Box<dyn Segment> {
        Self::new(&raw.unwrap_or_default(), &self.position_marker, WhitespaceSegmentNewArgs {})
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct UnlexableSegment {
    expected: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnlexableSegmentNewArgs {
    pub(crate) expected: Option<String>,
}

impl UnlexableSegment {
    pub fn new(
        _raw: &str,
        _position_maker: &PositionMarker,
        args: UnlexableSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(UnlexableSegment { expected: args.expected.unwrap_or("".to_string()) })
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
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SymbolSegment {
    raw: String,
    position_maker: PositionMarker,
    uuid: Uuid,
    type_: &'static str,
}

impl Segment for SymbolSegment {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self {
            raw: self.raw.clone(),
            position_maker: self.position_maker.clone(),
            uuid: self.uuid,
            type_: self.type_,
        }
        .boxed()
    }

    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
    }

    fn get_type(&self) -> &'static str {
        "symbol"
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

    fn instance_types(&self) -> HashSet<String> {
        HashSet::from([self.type_.to_string()])
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        let Some(position_marker) = position_marker else { return };
        self.position_maker = position_marker;
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
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
pub struct SymbolSegmentNewArgs {
    pub r#type: &'static str,
}

impl SymbolSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: SymbolSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(SymbolSegment {
            raw: raw.to_string(),
            position_maker: position_maker.clone(),
            uuid: Uuid::new_v4(),
            type_: args.r#type,
        })
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
            Some(PositionMarker::new(0..6, 0..6, template.clone(), None, None)),
            None,
            None,
            None,
            None,
            None,
            None,
        )) as Box<dyn Segment>;
        let rs2 = Box::new(RawSegment::new(
            Some("foobar".to_string()),
            Some(PositionMarker::new(0..6, 0..6, template.clone(), None, None)),
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
        let raw_seg = raw_seg();

        assert_eq!(raw_seg.get_raw().unwrap(), "foobar");
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

        let anchor_info = anchor_edit_info.get(&raw_segs[0].get_uuid().unwrap()).unwrap();

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
