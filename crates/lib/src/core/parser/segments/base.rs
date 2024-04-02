use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;

use ahash::AHashSet;
use dyn_clone::DynClone;
use dyn_hash::DynHash;
use dyn_ord::DynEq;
use itertools::{enumerate, Itertools};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::dialects::base::Dialect;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::fix::{AnchorEditInfo, FixPatch, SourceFix};
use crate::core::rules::base::{EditType, LintFix};
use crate::core::templaters::base::TemplatedFile;
use crate::dialects::ansi::{
    ColumnReferenceSegment, Node, ObjectReferenceSegment, WildcardIdentifierSegment,
};
use crate::helpers::ToErasedSegment;

/// An element of the response to BaseSegment.path_to().
///     Attributes:
///         segment (:obj:`BaseSegment`): The segment in the chain.
///         idx (int): The index of the target within its `segment`.
///         len (int): The number of children `segment` has.
#[derive(Debug, Clone)]
pub struct PathStep {
    pub segment: ErasedSegment,
    pub idx: usize,
    pub len: usize,
    pub code_idxs: Vec<usize>,
}

pub type SegmentConstructorFn<SegmentArgs> =
    &'static dyn Fn(&str, &PositionMarker, SegmentArgs) -> ErasedSegment;

pub trait CloneSegment {
    fn clone_box(&self) -> ErasedSegment;
}

impl<T: Segment> CloneSegment for T {
    fn clone_box(&self) -> ErasedSegment {
        dyn_clone::clone(self).to_erased_segment()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerialisedSegmentValue {
    Single(String),
    Nested(Vec<TupleSerialisedSegment>),
}

#[derive(Deserialize)]
pub struct TupleSerialisedSegment(String, SerialisedSegmentValue);

impl Serialize for TupleSerialisedSegment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_key(&self.0)?;
        map.serialize_value(&self.1)?;
        map.end()
    }
}

impl TupleSerialisedSegment {
    fn sinlge(key: String, value: String) -> Self {
        Self(key, SerialisedSegmentValue::Single(value))
    }

    fn nested(key: String, segments: Vec<TupleSerialisedSegment>) -> Self {
        Self(key, SerialisedSegmentValue::Nested(segments))
    }
}

#[derive(Debug, Hash, Clone)]
pub struct ErasedSegment {
    value: Rc<dyn Segment>,
}

impl ErasedSegment {
    fn deep_clone(&self) -> Self {
        self.clone_box()
    }

    #[track_caller]
    pub fn get_mut(&mut self) -> &mut dyn Segment {
        Rc::get_mut(&mut self.value).unwrap()
    }
}

impl Deref for ErasedSegment {
    type Target = dyn Segment;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref()
    }
}

impl PartialEq for ErasedSegment {
    fn eq(&self, other: &Self) -> bool {
        self.value.as_ref() == other.value.as_ref()
    }
}

impl ErasedSegment {
    pub fn of<T: Segment>(value: T) -> Self {
        Self { value: Rc::new(value) }
    }
}

pub trait Segment: Any + DynEq + DynClone + DynHash + Debug + CloneSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn as_object_reference(&self) -> Node<ObjectReferenceSegment> {
        if let Some(value) = self.as_any().downcast_ref::<Node<ObjectReferenceSegment>>() {
            return value.clone();
        }

        if let Some(value) = self.as_any().downcast_ref::<Node<WildcardIdentifierSegment>>() {
            let mut node = Node::new();
            node.uuid = value.uuid;
            node.position_marker = value.position_marker.clone();
            node.segments = value.segments.clone();
            return node;
        }

        if let Some(value) = self.as_any().downcast_ref::<Node<ColumnReferenceSegment>>() {
            let mut node = Node::new();
            node.uuid = value.uuid;
            node.position_marker = value.position_marker.clone();
            node.segments = value.segments.clone();
            return node;
        }

        unimplemented!("{} = {:?}", self.get_type(), self.get_raw())
    }

    fn get_start_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc(),
            None => unreachable!("{self:?} has no PositionMarker"),
        }
    }

    fn get_end_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc_after(&self.get_raw().unwrap()),
            None => {
                unreachable!("{self:?} has no PositionMarker")
            }
        }
    }

    fn to_serialised(
        &self,
        code_only: bool,
        show_raw: bool,
        include_meta: bool,
    ) -> TupleSerialisedSegment {
        if show_raw && self.segments().is_empty() {
            TupleSerialisedSegment::sinlge(self.get_type().into(), self.get_raw().unwrap())
        } else if code_only {
            let segments = self
                .segments()
                .into_iter()
                .filter(|seg| seg.is_code() && !seg.is_meta())
                .map(|seg| seg.to_serialised(code_only, show_raw, include_meta))
                .collect_vec();

            TupleSerialisedSegment::nested(self.get_type().into(), segments)
        } else {
            let segments = self
                .segments()
                .into_iter()
                .map(|seg| seg.to_serialised(code_only, show_raw, include_meta))
                .collect_vec();

            TupleSerialisedSegment::nested(self.get_type().into(), segments)
        }
    }

    fn select_children(
        &self,
        start_seg: Option<&ErasedSegment>,
        stop_seg: Option<&ErasedSegment>,
        select_if: Option<fn(&ErasedSegment) -> bool>,
        loop_while: Option<fn(&ErasedSegment) -> bool>,
    ) -> Vec<ErasedSegment> {
        let segments = self.segments();

        let start_index = start_seg
            .and_then(|seg| segments.iter().position(|x| x.dyn_eq(seg)))
            .map_or(0, |index| index + 1);

        let stop_index = stop_seg
            .and_then(|seg| segments.iter().position(|x| x.dyn_eq(seg)))
            .unwrap_or_else(|| segments.len());

        let mut buff = Vec::new();

        for seg in segments.iter().skip(start_index).take(stop_index - start_index) {
            if let Some(loop_while) = &loop_while {
                if !loop_while(seg) {
                    break;
                }
            }

            if select_if.as_ref().map_or(true, |f| f(seg)) {
                buff.push(seg.clone());
            }
        }

        buff
    }

    fn is_templated(&self) -> bool {
        if let Some(pos_marker) = self.get_position_marker() {
            pos_marker.source_slice.start != pos_marker.source_slice.end && !pos_marker.is_literal()
        } else {
            panic!("PosMarker must be set");
        }
    }

    fn iter_segments(&self, expanding: Option<&[&str]>, pass_through: bool) -> Vec<ErasedSegment> {
        let mut result = Vec::new();
        for s in self.gather_segments() {
            if let Some(expanding) = expanding {
                if expanding.iter().any(|ty| s.is_type(ty)) {
                    result.extend(
                        s.iter_segments(if pass_through { Some(expanding) } else { None }, false),
                    );
                } else {
                    result.push(s);
                }
            } else {
                result.push(s);
            }
        }
        result
    }

    fn recursive_crawl_all(&self, reverse: bool) -> Vec<ErasedSegment> {
        let mut result = Vec::new();

        if reverse {
            for seg in self.segments().iter().rev() {
                result.extend(seg.recursive_crawl_all(reverse));
            }
            result.push(self.clone_box());
        } else {
            result.push(self.clone_box());
            for seg in self.segments() {
                result.extend(seg.recursive_crawl_all(reverse));
            }
        }

        result
    }

    fn recursive_crawl(
        &self,
        seg_types: &[&str],
        recurse_into: bool,
        no_recursive_seg_type: Option<&str>,
        allow_self: bool,
    ) -> Vec<ErasedSegment> {
        let is_debug = seg_types == &["object_reference"];

        let mut acc = Vec::new();
        let seg_types_set: AHashSet<&str> = AHashSet::from_iter(seg_types.iter().copied());

        let matches =
            allow_self && self.class_types().iter().any(|it| seg_types_set.contains(it.as_str()));
        if matches {
            acc.push(self.clone_box());
        }

        if !self.descendant_type_set().iter().any(|ty| seg_types_set.contains(ty.as_str())) {
            return acc;
        }

        if recurse_into || !matches {
            for seg in self.segments() {
                if no_recursive_seg_type.map_or(true, |type_str| !seg.is_type(type_str)) {
                    let segments =
                        seg.recursive_crawl(seg_types, recurse_into, no_recursive_seg_type, true);
                    acc.extend(segments);
                }
            }
        }

        acc
    }

    fn code_indices(&self) -> Vec<usize> {
        self.segments()
            .iter()
            .enumerate()
            .filter(|(_, seg)| seg.is_code())
            .map(|(idx, _)| idx)
            .collect()
    }

    fn get_parent(&self) -> Option<ErasedSegment> {
        None
    }

    fn child(&self, seg_types: &[&str]) -> Option<ErasedSegment> {
        for seg in self.gather_segments() {
            if seg_types.iter().any(|ty| seg.is_type(ty)) {
                return Some(seg);
            }
        }
        None
    }

    fn children(&self, seg_types: &[&str]) -> Vec<ErasedSegment> {
        let mut buff = Vec::new();
        for seg in self.gather_segments() {
            if seg_types.iter().any(|ty| seg.is_type(ty)) {
                buff.push(seg);
            }
        }
        buff
    }

    fn path_to(&self, other: &ErasedSegment) -> Vec<PathStep> {
        let midpoint = other;

        for (idx, seg) in enumerate(self.segments()) {
            let mut steps = vec![PathStep {
                segment: self.clone_box(),
                idx,
                len: self.segments().len(),
                code_idxs: self.code_indices(),
            }];

            if seg.eq(&midpoint) {
                return steps;
            }

            let res = seg.path_to(midpoint);

            if !res.is_empty() {
                steps.extend(res);
                return steps;
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

    fn descendant_type_set(&self) -> AHashSet<String> {
        let mut result_set = AHashSet::new();

        for seg in self.segments() {
            result_set.extend(seg.descendant_type_set().union(&seg.class_types()).cloned());
        }

        result_set
    }

    // TODO: remove &self?
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        None
    }

    fn get_raw(&self) -> Option<String> {
        self.segments().iter().filter_map(|segment| segment.get_raw()).join("").into()
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
        self.segments().iter().any(|s| s.is_code())
    }
    fn is_comment(&self) -> bool {
        unimplemented!("{}", std::any::type_name::<Self>())
    }
    fn is_whitespace(&self) -> bool {
        false
    }
    fn is_meta(&self) -> bool {
        false
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        None
    }

    #[track_caller]
    fn get_position_marker(&self) -> Option<PositionMarker> {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn segments(&self) -> &[ErasedSegment] {
        unimplemented!()
    }

    // get_segments is the way the segment returns its children 'self.segments' in
    // Python.
    fn gather_segments(&self) -> Vec<ErasedSegment> {
        self.segments().to_vec()
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
    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        self.segments().into_iter().flat_map(|item| item.get_raw_segments()).collect_vec()
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

    /// Return any source fixes as list.
    fn get_source_fixes(&self) -> Vec<SourceFix> {
        self.segments().iter().flat_map(|seg| seg.get_source_fixes()).collect()
    }

    /// Stub.
    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
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

    fn instance_types(&self) -> AHashSet<String> {
        AHashSet::new()
    }

    fn combined_types(&self) -> AHashSet<String> {
        let mut combined = self.instance_types();
        combined.extend(self.class_types());
        combined
    }

    fn class_types(&self) -> AHashSet<String> {
        AHashSet::new()
    }

    #[allow(unused_variables)]
    fn apply_fixes(
        &self,
        dialect: Dialect,
        mut fixes: HashMap<Uuid, AnchorEditInfo>,
    ) -> (ErasedSegment, Vec<ErasedSegment>, Vec<ErasedSegment>, bool) {
        if fixes.is_empty() || self.segments().is_empty() {
            return (self.clone_box(), Vec::new(), Vec::new(), true);
        }

        let mut seg_buffer = Vec::new();
        let mut fixes_applied = Vec::new();
        let mut requires_validate = false;

        for seg in self.gather_segments() {
            // Look for uuid match.
            // This handles potential positioning ambiguity.

            let Some(anchor_info) = fixes.remove(&seg.get_uuid().unwrap()) else {
                seg_buffer.push(seg.clone());
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

                // Otherwise it must be a replace or a create.
                assert!(matches!(
                    f.edit_type,
                    EditType::Replace | EditType::CreateBefore | EditType::CreateAfter
                ));

                if f.edit_type == EditType::CreateAfter && anchor_info.fixes.len() == 1 {
                    // In the case of a creation after that is not part
                    // of a create_before/create_after pair, also add
                    // this segment before the edit.
                    seg_buffer.push(seg.clone());
                }

                for s in f.edit.as_ref().unwrap() {
                    seg_buffer.push(s.clone());
                }

                if !(f.edit_type == EditType::Replace
                    && f.edit.as_ref().map_or(false, |x| x.len() == 1)
                    && f.edit.as_ref().unwrap()[0].class_types() == seg.class_types())
                {
                    requires_validate = true;
                }

                if f.edit_type == EditType::CreateBefore {
                    seg_buffer.push(seg.clone());
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

    fn raw_segments_with_ancestors(&self) -> Vec<(ErasedSegment, Vec<PathStep>)> {
        let mut buffer: Vec<(ErasedSegment, Vec<PathStep>)> = Vec::new();
        let code_idxs: Vec<usize> = self.code_indices();

        for (idx, seg) in self.segments().iter().enumerate() {
            let mut new_step = vec![PathStep {
                segment: self.clone_box(),
                idx,
                len: self.segments().len(),
                code_idxs: code_idxs.clone(),
            }];

            // Use seg.get_segments().is_empty() as a workaround to check if the segment is
            // a "raw" type. In the original Python code, this was achieved
            // using seg.is_type("raw"). Here, we assume that a "raw" segment is
            // characterized by having no sub-segments.

            if seg.segments().is_empty() {
                buffer.push((seg.clone(), new_step));
            } else {
                let mut extended = seg
                    .raw_segments_with_ancestors()
                    .into_iter()
                    .map(|(raw_seg, stack)| {
                        let mut new_step = new_step.clone();
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

impl PartialEq for dyn Segment {
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

impl Eq for ErasedSegment {}

pub fn position_segments(
    segments: &[ErasedSegment],
    parent_pos: Option<&PositionMarker>,
    metas_only: bool,
) -> Vec<ErasedSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let (mut line_no, mut line_pos) = match parent_pos {
        Some(pos) => (pos.working_line_no, pos.working_line_pos),
        None => {
            // Infer from first segment with a position
            segments
                .iter()
                .find_map(|seg| {
                    seg.get_position_marker()
                        .as_ref()
                        .map(|pm| (pm.working_line_no, pm.working_line_pos))
                })
                .expect("Unable to find working position")
        }
    };

    let mut segment_buffer = Vec::new();

    for (idx, segment) in enumerate(segments) {
        if metas_only && !segment.is_meta() {
            segment_buffer.push(segment.clone());
            (line_no, line_pos) =
                PositionMarker::infer_next_position(&segment.get_raw().unwrap(), line_no, line_pos);
            continue;
        }

        let old_position = segment.get_position_marker();
        let mut new_position = segment.get_position_marker();

        if old_position.is_none() {
            let mut start_point = None;

            if idx > 0 {
                let prev_seg = segment_buffer[idx - 1].clone();
                start_point = prev_seg.get_position_marker().unwrap().into();
            } else if let Some(parent_pos) = parent_pos {
                start_point = parent_pos.start_point_marker().into();
            }

            let end_point = segments.iter().skip(idx + 1).find_map(|fwd_seg| {
                fwd_seg.get_position_marker().map(|_| {
                    fwd_seg.get_raw_segments()[0]
                        .get_position_marker()
                        .unwrap()
                        .start_point_marker()
                })
            });

            if let Some((start_point, end_point)) = start_point.as_ref().zip(end_point.as_ref())
                && start_point != end_point
            {
                new_position = PositionMarker::from_points(start_point, end_point).into();
            } else if let Some(start_point) = start_point.as_ref() {
                new_position = start_point.clone().into();
            } else if let Some(end_point) = end_point.as_ref() {
                new_position = end_point.clone().into();
            } else {
                unimplemented!("Unable to position new segment");
            }

            assert_ne!(new_position, None);
        }

        let new_seg = if !segment.segments().is_empty() && old_position != new_position {
            unimplemented!()
        } else {
            let mut new_seg = segment.deep_clone();
            new_seg.get_mut().set_position_marker(new_position);
            new_seg
        };

        segment_buffer.push(new_seg);
    }

    pretty_assertions::assert_eq!(segments, segment_buffer);

    segment_buffer
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
    ) -> ErasedSegment {
        CodeSegment {
            raw: raw.to_string(),
            position_marker: Some(position_maker.clone()),
            code_type: args.code_type,
            instance_types: vec![],
            trim_start: None,
            trim_chars: None,
            source_fixes: None,
            uuid: Uuid::new_v4(),
        }
        .to_erased_segment()
    }
}

impl Segment for CodeSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn class_types(&self) -> AHashSet<String> {
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

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
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
    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
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

#[derive(Debug, Hash, Clone, PartialEq)]
pub struct IdentifierSegment {
    base: CodeSegment,
}

impl IdentifierSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: CodeSegmentNewArgs,
    ) -> ErasedSegment {
        IdentifierSegment {
            base: CodeSegment {
                raw: raw.to_string(),
                position_marker: Some(position_maker.clone()),
                code_type: args.code_type,
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
                uuid: Uuid::new_v4(),
            },
        }
        .to_erased_segment()
    }
}

impl Segment for IdentifierSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn class_types(&self) -> AHashSet<String> {
        Some(self.get_type().to_owned()).into_iter().collect()
    }

    fn get_raw(&self) -> Option<String> {
        Some(self.base.raw.clone())
    }
    fn get_type(&self) -> &'static str {
        "identifier"
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
        self.base.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.base.position_marker = position_marker
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.base.uuid.into()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn edit(&self, raw: Option<String>, source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        IdentifierSegment::new(
            raw.unwrap_or(self.base.raw.clone()).as_str(),
            &self.base.position_marker.clone().unwrap(),
            CodeSegmentNewArgs {
                code_type: self.base.code_type,
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
    position_maker: Option<PositionMarker>,
    r#type: &'static str,
    trim_start: Vec<&'static str>,
    uuid: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentSegmentNewArgs {
    pub r#type: &'static str,
    pub trim_start: Option<Vec<&'static str>>,
}

impl CommentSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: CommentSegmentNewArgs,
    ) -> ErasedSegment {
        Self {
            raw: raw.to_string(),
            position_maker: position_maker.clone().into(),
            r#type: args.r#type,
            trim_start: args.trim_start.unwrap_or_default(),
            uuid: Uuid::new_v4(),
        }
        .to_erased_segment()
    }
}

impl Segment for CommentSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone_box()
    }

    fn get_type(&self) -> &'static str {
        "comment"
    }

    fn class_types(&self) -> AHashSet<String> {
        ["comment".into()].into()
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
        self.uuid.into()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker;
    }

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
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

#[derive(Debug, Clone, Default)]
pub struct NewlineSegmentNewArgs {}

impl NewlineSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        _args: NewlineSegmentNewArgs,
    ) -> ErasedSegment {
        NewlineSegment {
            raw: raw.to_string(),
            position_maker: position_maker.clone(),
            uuid: Uuid::new_v4(),
        }
        .to_erased_segment()
    }
}

impl Segment for NewlineSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone_box()]
    }

    fn is_meta(&self) -> bool {
        false
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

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        let Some(position_marker) = position_marker else {
            return;
        };

        dbg!("self.position_marker = position_marker;");
    }

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_raw(&self) -> Option<String> {
        Some(self.raw.clone())
    }

    fn class_types(&self) -> AHashSet<String> {
        ["newline".into()].into()
    }
}

/// Segment containing whitespace.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct WhitespaceSegment {
    raw: String,
    position_marker: PositionMarker,
    uuid: Uuid,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct WhitespaceSegmentNewArgs;

impl WhitespaceSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        _args: WhitespaceSegmentNewArgs,
    ) -> ErasedSegment {
        WhitespaceSegment {
            raw: raw.to_string(),
            position_marker: position_maker.clone(),
            uuid: Uuid::new_v4(),
        }
        .to_erased_segment()
    }
}

impl Segment for WhitespaceSegment {
    fn new(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self {
            raw: self.get_raw().unwrap(),
            position_marker: self.position_marker.clone(),
            uuid: self.uuid.clone(),
        }
        .to_erased_segment()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
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

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        let Some(position_marker) = position_marker else {
            return;
        };

        self.position_marker = position_marker;
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn edit(&self, raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
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
    ) -> ErasedSegment {
        UnlexableSegment { expected: args.expected.unwrap_or("".to_string()) }.to_erased_segment()
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

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
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
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self {
            raw: self.raw.clone(),
            position_maker: self.position_maker.clone(),
            uuid: self.uuid,
            type_: self.type_,
        }
        .to_erased_segment()
    }

    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_type(&self) -> &'static str {
        self.type_
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

    fn instance_types(&self) -> AHashSet<String> {
        [self.type_.to_string()].into()
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

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        todo!()
    }
}

#[derive(Debug, Default, Clone)]
pub struct SymbolSegmentNewArgs {
    pub r#type: &'static str,
}

impl SymbolSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: SymbolSegmentNewArgs,
    ) -> ErasedSegment {
        SymbolSegment {
            raw: raw.to_string(),
            position_maker: position_maker.clone(),
            uuid: Uuid::new_v4(),
            type_: args.r#type,
        }
        .to_erased_segment()
    }
}

#[derive(Debug, Hash, Clone, PartialEq)]
pub struct UnparsableSegment {
    uuid: Uuid,
    pub segments: Vec<ErasedSegment>,
    position_marker: Option<PositionMarker>,
}

impl UnparsableSegment {
    pub fn new(segments: Vec<ErasedSegment>) -> Self {
        let mut this = Self { uuid: Uuid::new_v4(), segments, position_marker: None };
        this.uuid = Uuid::new_v4();
        this.set_position_marker(pos_marker(&this).into());
        this
    }
}

impl Segment for UnparsableSegment {
    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &self.segments
    }

    fn get_type(&self) -> &'static str {
        "unparsable"
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }
}

pub fn pos_marker(this: &dyn Segment) -> PositionMarker {
    let markers: Vec<_> =
        this.segments().into_iter().flat_map(|seg| seg.get_position_marker()).collect();

    PositionMarker::from_child_markers(markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::segments::raw::RawSegment;
    use crate::core::parser::segments::test_functions::{raw_seg, raw_segments};

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
        ))
        .to_erased_segment();

        let rs2 = Box::new(RawSegment::new(
            Some("foobar".to_string()),
            Some(PositionMarker::new(0..6, 0..6, template.clone(), None, None)),
            None,
            None,
            None,
            None,
            None,
            None,
        ))
        .to_erased_segment();

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
