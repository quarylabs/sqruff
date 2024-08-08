use std::any::Any;
use std::borrow::Cow;
use std::cell::OnceCell;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use ahash::AHashMap;
use dyn_clone::DynClone;
use dyn_ord::DynEq;
use itertools::{enumerate, Itertools};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::core::dialects::init::DialectKind;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::fix::{AnchorEditInfo, FixPatch, SourceFix};
use crate::core::rules::base::{EditType, LintFix};
use crate::core::templaters::base::TemplatedFile;
use crate::dialects::ansi::{ObjectReferenceKind, ObjectReferenceSegment};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::{Config, ToErasedSegment};

#[derive(Debug, Clone)]
pub struct PathStep {
    pub segment: ErasedSegment,
    pub idx: usize,
    pub len: usize,
    pub code_idxs: Rc<[usize]>,
}

pub type SegmentConstructorFn<SegmentArgs> =
    &'static (dyn Fn(&str, Option<PositionMarker>, SegmentArgs) -> ErasedSegment + Sync + Send);

pub trait CloneSegment {
    #[track_caller]
    fn clone_box(&self) -> ErasedSegment;
}

impl<T: Segment> CloneSegment for T {
    #[track_caller]
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

#[derive(Debug, Clone)]
pub struct ErasedSegment {
    value: Rc<dyn Segment>,
    hash: Rc<AtomicU64>,
}

impl Hash for ErasedSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_value().hash(state);
    }
}

impl ErasedSegment {
    pub fn is(&self, other: &ErasedSegment) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }

    pub fn addr(&self) -> usize {
        fn addr<T: ?Sized>(t: *const T) -> usize {
            let c: *const () = t.cast();
            sptr::Strict::addr(c)
        }

        addr(Rc::as_ptr(&self.value))
    }

    pub fn direct_descendant_type_set(&self) -> SyntaxSet {
        self.segments().iter().fold(SyntaxSet::EMPTY, |set, it| set.union(&it.class_types()))
    }

    pub(crate) fn is_keyword(&self, p0: &str) -> bool {
        self.is_type(SyntaxKind::Keyword) && self.raw().eq_ignore_ascii_case(p0)
    }

    pub fn hash_value(&self) -> u64 {
        let mut hash = self.hash.load(Ordering::Acquire);

        if hash == 0 {
            let mut hasher = ahash::AHasher::default();
            self.value.hash(&mut hasher);
            hash = hasher.finish();

            let exchange = self.hash.compare_exchange(0, hash, Ordering::AcqRel, Ordering::Acquire);
            if let Err(old) = exchange {
                hash = old
            }
        }

        hash
    }

    pub fn deep_clone(&self) -> Self {
        self.clone_box()
    }

    #[track_caller]
    pub fn get_mut(&mut self) -> &mut dyn Segment {
        Rc::get_mut(&mut self.value).unwrap()
    }

    #[track_caller]
    pub fn make_mut(&mut self) -> &mut dyn Segment {
        let mut this = self.deep_clone();
        std::mem::swap(self, &mut this);
        Rc::get_mut(&mut self.value).unwrap()
    }

    pub fn reference(&self) -> ObjectReferenceSegment {
        ObjectReferenceSegment(
            self.clone(),
            match self.get_type() {
                SyntaxKind::TableReference => ObjectReferenceKind::Table,
                SyntaxKind::WildcardIdentifier => ObjectReferenceKind::WildcardIdentifier,
                _ => ObjectReferenceKind::Object,
            },
        )
    }

    pub fn recursive_crawl_all(&self, reverse: bool) -> Vec<ErasedSegment> {
        let mut result = Vec::new();

        if reverse {
            for seg in self.segments().iter().rev() {
                result.extend(seg.recursive_crawl_all(reverse));
            }
            result.push(self.clone());
        } else {
            result.push(self.clone());
            for seg in self.segments() {
                result.extend(seg.recursive_crawl_all(reverse));
            }
        }

        result
    }

    pub fn recursive_crawl(
        &self,
        types: SyntaxSet,
        recurse_into: bool,
        no_recursive_types: Option<SyntaxSet>,
        allow_self: bool,
    ) -> Vec<ErasedSegment> {
        let mut acc = Vec::new();

        let matches = allow_self && self.class_types().intersects(&types);
        if matches {
            acc.push(self.clone());
        }

        if !self.descendant_type_set().intersects(&types) {
            return acc;
        }

        if recurse_into || !matches {
            for seg in self.segments() {
                if no_recursive_types
                    .map_or(true, |no_recursive_types| !no_recursive_types.contains(seg.get_type()))
                {
                    let segments =
                        seg.recursive_crawl(types, recurse_into, no_recursive_types, true);
                    acc.extend(segments);
                }
            }
        }

        acc
    }

    pub fn raw_segments_with_ancestors(&self) -> &[(ErasedSegment, Vec<PathStep>)] {
        self.value.raw_segments_with_ancestors().get_or_init(|| {
            let mut buffer: Vec<(ErasedSegment, Vec<PathStep>)> =
                Vec::with_capacity(self.segments().len());
            let code_idxs: Rc<[usize]> = self.code_indices().into();

            for (idx, seg) in self.segments().iter().enumerate() {
                let new_step = vec![PathStep {
                    segment: self.clone(),
                    idx,
                    len: self.segments().len(),
                    code_idxs: code_idxs.clone(),
                }];

                // Use seg.get_segments().is_empty() as a workaround to check if the segment is
                // a SyntaxKind::Raw type. In the original Python code, this was achieved
                // using seg.is_type(SyntaxKind::Raw). Here, we assume that a SyntaxKind::Raw
                // segment is characterized by having no sub-segments.

                if seg.segments().is_empty() {
                    buffer.push((seg.clone(), new_step));
                } else {
                    let extended =
                        seg.raw_segments_with_ancestors().iter().map(|(raw_seg, stack)| {
                            let mut new_step = new_step.clone();
                            new_step.extend_from_slice(stack);
                            (raw_seg.clone(), new_step)
                        });

                    buffer.extend(extended);
                }
            }

            buffer
        })
    }

    pub fn path_to(&self, other: &ErasedSegment) -> Vec<PathStep> {
        let midpoint = other;

        for (idx, seg) in enumerate(self.segments()) {
            let mut steps = vec![PathStep {
                segment: self.clone(),
                idx,
                len: self.segments().len(),
                code_idxs: self.code_indices().into(),
            }];

            if seg.eq(midpoint) {
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

    pub fn apply_fixes(
        &self,
        fixes: &mut AHashMap<u32, AnchorEditInfo>,
    ) -> (ErasedSegment, Vec<ErasedSegment>, Vec<ErasedSegment>, bool) {
        if fixes.is_empty() || self.segments().is_empty() {
            return (self.clone(), Vec::new(), Vec::new(), true);
        }

        let mut seg_buffer = Vec::new();
        let mut fixes_applied = Vec::new();
        let mut _requires_validate = false;

        for seg in self.gather_segments() {
            // Look for uuid match.
            // This handles potential positioning ambiguity.

            let Some(mut anchor_info) = fixes.remove(&seg.id()) else {
                seg_buffer.push(seg.clone());
                continue;
            };

            if anchor_info.fixes.len() == 2
                && anchor_info.fixes[0].edit_type == EditType::CreateAfter
            {
                anchor_info.fixes.reverse();
            }

            let fixes_count = anchor_info.fixes.len();
            for mut f in anchor_info.fixes {
                fixes_applied.push(f.clone());

                // Deletes are easy.
                #[allow(unused_assignments)]
                if f.edit_type == EditType::Delete {
                    // We're just getting rid of this segment.
                    _requires_validate = true;
                    // NOTE: We don't add the segment in this case.
                    continue;
                }

                // Otherwise it must be a replace or a create.
                assert!(matches!(
                    f.edit_type,
                    EditType::Replace | EditType::CreateBefore | EditType::CreateAfter
                ));

                if f.edit_type == EditType::CreateAfter && fixes_count == 1 {
                    // In the case of a creation after that is not part
                    // of a create_before/create_after pair, also add
                    // this segment before the edit.
                    seg_buffer.push(seg.clone());
                }

                let mut consumed_pos = false;
                for s in std::mem::take(f.edit.as_mut().unwrap()) {
                    let mut s = s.deep_clone();
                    if f.edit_type == EditType::Replace && !consumed_pos && s.raw() == seg.raw() {
                        consumed_pos = true;
                        s.get_mut().set_position_marker(seg.get_position_marker());
                    }

                    seg_buffer.push(s);
                }

                #[allow(unused_assignments)]
                if !(f.edit_type == EditType::Replace
                    && f.edit.as_ref().map_or(false, |x| x.len() == 1)
                    && f.edit.as_ref().unwrap()[0].class_types() == seg.class_types())
                {
                    _requires_validate = true;
                }

                if f.edit_type == EditType::CreateBefore {
                    seg_buffer.push(seg.clone());
                }
            }
        }

        if !fixes_applied.is_empty() {
            seg_buffer =
                position_segments(&seg_buffer, self.get_position_marker().as_ref().unwrap());
        }

        let seg_queue = seg_buffer;
        let mut seg_buffer = Vec::new();
        for seg in seg_queue {
            let (s, pre, post, validated) = seg.apply_fixes(fixes);

            seg_buffer.extend(pre);
            seg_buffer.push(s);
            seg_buffer.extend(post);

            #[allow(unused_assignments)]
            if !validated {
                _requires_validate = true;
            }
        }

        let seg_buffer =
            position_segments(&seg_buffer, self.get_position_marker().as_ref().unwrap());
        (self.new(seg_buffer), Vec::new(), Vec::new(), false)
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
        Self { value: Rc::new(value), hash: Rc::new(AtomicU64::new(0)) }
    }
}

pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait Segment: Any + AsAny + DynClone + Debug + CloneSegment {
    #[allow(clippy::new_ret_no_self, clippy::wrong_self_convention)]
    #[track_caller]
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn copy(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        todo!("{}", std::any::type_name::<Self>())
    }

    fn can_start_end_non_code(&self) -> bool {
        false
    }

    #[track_caller]
    fn dialect(&self) -> DialectKind {
        todo!("{}", std::any::type_name::<Self>())
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn get_start_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc(),
            None => unreachable!("{self:?} has no PositionMarker"),
        }
    }

    fn get_end_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc_after(&self.raw()),
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
            TupleSerialisedSegment::sinlge(
                self.get_type().as_str().to_string(),
                self.raw().to_string(),
            )
        } else if code_only {
            let segments = self
                .segments()
                .iter()
                .filter(|seg| seg.is_code() && !seg.is_meta())
                .map(|seg| seg.to_serialised(code_only, show_raw, include_meta))
                .collect_vec();

            TupleSerialisedSegment::nested(self.get_type().as_str().to_string(), segments)
        } else {
            let segments = self
                .segments()
                .iter()
                .map(|seg| seg.to_serialised(code_only, show_raw, include_meta))
                .collect_vec();

            TupleSerialisedSegment::nested(self.get_type().as_str().to_string(), segments)
        }
    }

    fn raw_segments_with_ancestors(&self) -> &OnceCell<Vec<(ErasedSegment, Vec<PathStep>)>> {
        todo!("{}", std::any::type_name::<Self>())
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
            .unwrap_or(segments.len());

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

    fn iter_segments(
        &self,
        expanding: Option<SyntaxSet>,
        pass_through: bool,
    ) -> Vec<ErasedSegment> {
        let mut result = Vec::new();
        for s in self.gather_segments() {
            if let Some(expanding) = expanding {
                if expanding.contains(s.get_type()) {
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

    fn child(&self, seg_types: SyntaxSet) -> Option<ErasedSegment> {
        self.gather_segments().into_iter().find(|seg| seg_types.contains(seg.get_type()))
    }

    fn children(&self, seg_types: SyntaxSet) -> Vec<ErasedSegment> {
        let mut buff = Vec::new();
        for seg in self.gather_segments() {
            if seg_types.contains(seg.get_type()) {
                buff.push(seg);
            }
        }
        buff
    }

    fn iter_patches(&self, templated_file: &TemplatedFile) -> Vec<FixPatch> {
        let mut acc = Vec::new();

        if self.get_position_marker().is_none() {
            return Vec::new();
        }

        if self.get_position_marker().unwrap().is_literal() {
            acc.extend(self.iter_source_fix_patches(templated_file));
            acc.push(FixPatch::new(
                self.get_position_marker().unwrap().templated_slice,
                self.raw().into(),
                // SyntaxKind::Literal.into(),
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

    fn descendant_type_set(&self) -> &SyntaxSet {
        const { &SyntaxSet::EMPTY }
    }

    fn raw(&self) -> Cow<str> {
        self.segments().iter().map(|segment| segment.raw()).join("").into()
    }

    fn get_raw_upper(&self) -> Option<String> {
        self.raw().to_uppercase().into()
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

    fn get_type(&self) -> SyntaxKind {
        todo!()
    }
    fn is_type(&self, type_: SyntaxKind) -> bool {
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

    #[allow(unused_variables)]
    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn segments(&self) -> &[ErasedSegment] {
        unimplemented!()
    }

    #[track_caller]
    fn set_segments(&mut self, _segments: Vec<ErasedSegment>) {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    // get_segments is the way the segment returns its children 'self.segments' in
    // Python.
    fn gather_segments(&self) -> Vec<ErasedSegment> {
        self.segments().to_vec()
    }

    /// Return the length of the segment in characters.
    fn get_matched_length(&self) -> usize {
        self.raw().len()
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
        self.segments().iter().flat_map(|item| item.get_raw_segments()).collect_vec()
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
                // String::from("source"),
                source_fix.source_slice.clone(),
                templated_file.templated_str.clone().unwrap()[source_fix.templated_slice.clone()]
                    .to_string(),
                templated_file.source_str[source_fix.source_slice.clone()].to_string(),
            ));
        }
        patches
    }

    fn id(&self) -> u32 {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn set_id(&mut self, id: u32) {
        _ = id;
        todo!("{}", std::any::type_name::<Self>())
    }

    /// Return any source fixes as list.
    fn get_source_fixes(&self) -> Vec<SourceFix> {
        self.segments().iter().flat_map(|seg| seg.get_source_fixes()).collect()
    }

    /// Stub.
    #[allow(unused_variables)]
    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    /// Group and count fixes by anchor, return dictionary.
    fn compute_anchor_edit_info(&self, fixes: &Vec<LintFix>) -> AHashMap<u32, AnchorEditInfo> {
        let mut anchor_info = AHashMap::<u32, AnchorEditInfo>::new();
        for fix in fixes {
            // :TRICKY: Use segment uuid as the dictionary key since
            // different segments may compare as equal.
            let anchor_id = fix.anchor.id();
            anchor_info.entry(anchor_id).or_default().add(fix.clone());
        }
        anchor_info
    }

    fn instance_types(&self) -> SyntaxSet {
        SyntaxSet::EMPTY
    }

    fn combined_types(&self) -> SyntaxSet {
        self.instance_types().union(&self.class_types())
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::EMPTY
    }
}

dyn_clone::clone_trait_object!(Segment);

impl PartialEq for dyn Segment {
    fn eq(&self, other: &Self) -> bool {
        if self.id() == other.id() {
            return true;
        }

        let pos_self = self.get_position_marker();
        let pos_other = other.get_position_marker();
        if let Some((pos_self, pos_other)) = pos_self.zip(pos_other) {
            self.get_type() == other.get_type()
                && pos_self.working_loc() == pos_other.working_loc()
                && self.raw() == other.raw()
        } else {
            false
        }
    }
}

impl Eq for ErasedSegment {}

impl Hash for dyn Segment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get_type().hash(state);
        self.raw().hash(state);

        if let Some(marker) = &self.get_position_marker() {
            marker.source_position().hash(state);
        } else {
            None::<usize>.hash(state);
        }
    }
}

pub fn position_segments(
    segments: &[ErasedSegment],
    parent_pos: &PositionMarker,
) -> Vec<ErasedSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let (mut line_no, mut line_pos) = { (parent_pos.working_line_no, parent_pos.working_line_pos) };

    let mut segment_buffer: Vec<ErasedSegment> = Vec::new();
    for (idx, segment) in enumerate(segments) {
        let old_position = segment.get_position_marker();

        let mut new_position = match old_position.clone() {
            Some(pos_marker) => pos_marker.clone(),
            None => {
                let start_point = if idx > 0 {
                    let prev_seg = segment_buffer[idx - 1].clone();
                    Some(prev_seg.get_position_marker().unwrap().end_point_marker())
                } else {
                    Some(parent_pos.start_point_marker())
                };

                let mut end_point = None;
                for fwd_seg in &segments[idx + 1..] {
                    if fwd_seg.get_position_marker().is_some() {
                        end_point = Some(
                            fwd_seg.get_raw_segments()[0]
                                .get_position_marker()
                                .unwrap()
                                .start_point_marker(),
                        );
                        break;
                    }
                }

                if let Some((start_point, end_point)) = start_point.as_ref().zip(end_point.as_ref())
                    && start_point != end_point
                {
                    PositionMarker::from_points(start_point, end_point)
                } else if let Some(start_point) = start_point.as_ref() {
                    start_point.clone()
                } else if let Some(end_point) = end_point.as_ref() {
                    end_point.clone()
                } else {
                    unimplemented!("Unable to position new segment")
                }
            }
        };

        new_position = new_position.with_working_position(line_no, line_pos);
        (line_no, line_pos) =
            PositionMarker::infer_next_position(&segment.raw(), line_no, line_pos);

        let mut new_seg =
            if !segment.segments().is_empty() && old_position.as_ref() != Some(&new_position) {
                let child_segments = position_segments(segment.segments(), &new_position);
                segment.copy(child_segments)
            } else {
                segment.deep_clone()
            };

        new_seg.get_mut().set_position_marker(new_position.into());
        segment_buffer.push(new_seg);
    }

    segment_buffer
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeSegment {
    raw: SmolStr,
    position_marker: Option<PositionMarker>,
    pub code_type: SyntaxKind,

    // From RawSegment
    id: u32,
    instance_types: Vec<String>,
    trim_start: Option<Vec<String>>,
    trim_chars: Option<Vec<String>>,
    source_fixes: Option<Vec<SourceFix>>,
}

#[derive(Debug, Clone, Default)]
pub struct CodeSegmentNewArgs {
    pub code_type: SyntaxKind,

    pub instance_types: Vec<String>,
    pub trim_start: Option<Vec<String>>,
    pub trim_chars: Option<Vec<String>>,
    pub source_fixes: Option<Vec<SourceFix>>,
}

impl CodeSegment {
    pub fn create(
        raw: &str,
        position_maker: Option<PositionMarker>,
        args: CodeSegmentNewArgs,
    ) -> ErasedSegment {
        CodeSegment {
            raw: raw.into(),
            position_marker: position_maker,
            code_type: args.code_type,
            instance_types: vec![],
            trim_start: None,
            trim_chars: None,
            source_fixes: None,
            id: 0,
        }
        .to_erased_segment()
    }
}

impl Segment for CodeSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
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

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
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
    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        CodeSegment::create(
            raw.unwrap_or(self.raw.to_string()).as_str(),
            self.position_marker.clone(),
            CodeSegmentNewArgs {
                code_type: self.code_type,
                instance_types: vec![],
                trim_start: None,
                source_fixes,
                trim_chars: None,
            },
        )
        .config(|this| this.get_mut().set_id(id))
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::single(self.get_type())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdentifierSegment {
    base: CodeSegment,
}

impl IdentifierSegment {
    pub fn create(
        id: u32,
        raw: &str,
        position_maker: Option<PositionMarker>,
        args: CodeSegmentNewArgs,
    ) -> ErasedSegment {
        IdentifierSegment {
            base: CodeSegment {
                raw: raw.into(),
                position_marker: position_maker.clone(),
                code_type: args.code_type,
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
                id,
            },
        }
        .to_erased_segment()
    }
}

impl Segment for IdentifierSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.base.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::NakedIdentifier
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

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.base.id
    }

    fn set_id(&mut self, id: u32) {
        self.base.id = id;
    }

    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        IdentifierSegment::create(
            id,
            raw.unwrap_or(self.base.raw.to_string()).as_str(),
            self.base.position_marker.clone(),
            CodeSegmentNewArgs {
                code_type: self.base.code_type,
                instance_types: vec![],
                trim_start: None,
                source_fixes,
                trim_chars: None,
            },
        )
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::single(self.get_type())
    }
}

/// Segment containing a comment.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentSegment {
    raw: SmolStr,
    position_maker: Option<PositionMarker>,
    r#type: SyntaxKind,
    trim_start: Vec<&'static str>,
    id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentSegmentNewArgs {
    pub r#type: SyntaxKind,
    pub trim_start: Option<Vec<&'static str>>,
}

impl CommentSegment {
    pub fn create(
        raw: &str,
        position_maker: Option<PositionMarker>,
        args: CommentSegmentNewArgs,
    ) -> ErasedSegment {
        Self {
            raw: raw.into(),
            position_maker,
            r#type: args.r#type,
            trim_start: args.trim_start.unwrap_or_default(),
            id: 0,
        }
        .to_erased_segment()
    }
}

impl Segment for CommentSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone_box()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        self.r#type
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn edit(
        &self,
        _id: u32,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        todo!()
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::single(self.r#type)
    }
}

// Segment containing a newline.
#[derive(Debug, Clone, PartialEq)]
pub struct NewlineSegment {
    raw: SmolStr,
    position_maker: Option<PositionMarker>,
    id: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NewlineSegmentNewArgs {}

impl NewlineSegment {
    pub fn create(
        id: u32,
        raw: &str,
        position_maker: Option<PositionMarker>,
        _args: NewlineSegmentNewArgs,
    ) -> ErasedSegment {
        NewlineSegment { raw: raw.into(), position_maker: position_maker.clone(), id }
            .to_erased_segment()
    }
}

impl Segment for NewlineSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        self.clone().to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Newline
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
    fn is_meta(&self) -> bool {
        false
    }
    fn get_default_raw(&self) -> Option<&'static str> {
        Some("\n")
    }
    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone_box()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn edit(
        &self,
        _id: u32,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        todo!()
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::new(&[SyntaxKind::Newline])
    }
}

/// Segment containing whitespace.
#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceSegment {
    raw: SmolStr,
    position_marker: Option<PositionMarker>,
    id: u32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct WhitespaceSegmentNewArgs;

impl WhitespaceSegment {
    pub fn create(
        id: u32,
        raw: &str,
        position_maker: Option<PositionMarker>,
        _args: WhitespaceSegmentNewArgs,
    ) -> ErasedSegment {
        WhitespaceSegment { raw: raw.into(), position_marker: position_maker, id }
            .to_erased_segment()
    }
}

impl Segment for WhitespaceSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { raw: self.raw().into(), position_marker: self.position_marker.clone(), id: self.id }
            .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Whitespace
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
        self.position_marker.clone()
    }
    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        Self::create(
            id,
            &raw.unwrap_or_default(),
            self.position_marker.clone(),
            WhitespaceSegmentNewArgs {},
        )
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
    pub fn create(
        _raw: &str,
        _position_maker: Option<PositionMarker>,
        args: UnlexableSegmentNewArgs,
    ) -> ErasedSegment {
        UnlexableSegment { expected: args.expected.unwrap_or("".to_string()) }.to_erased_segment()
    }
}

impl Segment for UnlexableSegment {
    fn raw(&self) -> Cow<str> {
        todo!()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Unlexable
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

    fn id(&self) -> u32 {
        todo!()
    }

    fn edit(
        &self,
        _id: u32,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        todo!()
    }
}

/// A segment used for matching single entities which aren't keywords.
///
///     We rename the segment class here so that descendants of
///     _ProtoKeywordSegment can use the same functionality
///     but don't end up being labelled as a `keyword` later.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolSegment {
    raw: SmolStr,
    position_maker: Option<PositionMarker>,
    id: u32,
    type_: SyntaxKind,
}

impl Segment for SymbolSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self {
            raw: self.raw.clone(),
            position_maker: self.position_maker.clone(),
            id: self.id,
            type_: self.type_,
        }
        .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        SymbolSegment::create(id, &raw.unwrap(), self.position_maker.clone(), <_>::default())
    }

    fn instance_types(&self) -> SyntaxSet {
        SyntaxSet::single(self.type_)
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::new(&[self.get_type()])
    }
}

#[derive(Debug, Default, Clone)]
pub struct SymbolSegmentNewArgs {
    pub r#type: SyntaxKind,
}

impl SymbolSegment {
    pub fn create(
        id: u32,
        raw: &str,
        position_maker: Option<PositionMarker>,
        args: SymbolSegmentNewArgs,
    ) -> ErasedSegment {
        SymbolSegment {
            raw: raw.into(),
            position_maker: position_maker.clone(),
            id,
            type_: args.r#type,
        }
        .to_erased_segment()
    }
}

#[track_caller]
pub fn pos_marker(segments: &[ErasedSegment]) -> PositionMarker {
    let markers = segments.iter().flat_map(|seg| seg.get_position_marker());

    PositionMarker::from_child_markers(markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::segments::raw::{RawSegment, RawSegmentArgs};
    use crate::core::parser::segments::test_functions::{raw_seg, raw_segments};
    use crate::dialects::ansi::Tables;

    const TEMP_SEGMENTS_ARGS: RawSegmentArgs = RawSegmentArgs {
        _type: None,
        _instance_types: None,
        _source_fixes: None,
        _trim_cars: None,
        _trim_start: None,
        _uuid: None,
    };

    #[test]
    /// Test comparison of raw segments.
    fn test_parser_base_segments_raw_compare() {
        let template = TemplatedFile::from_string("foobar".to_string());
        let rs1 = Box::new(RawSegment::create(
            Some("foobar".to_string()),
            Some(PositionMarker::new(0..6, 0..6, template.clone(), None, None)),
            TEMP_SEGMENTS_ARGS,
        ))
        .to_erased_segment();

        let rs2 = Box::new(RawSegment::create(
            Some("foobar".to_string()),
            Some(PositionMarker::new(0..6, 0..6, template.clone(), None, None)),
            TEMP_SEGMENTS_ARGS,
        ))
        .to_erased_segment();

        assert_eq!(rs1, rs2)
    }

    #[test]
    // TODO Implement
    /// Test raw segments behave as expected.
    fn test_parser_base_segments_raw() {
        let raw_seg = raw_seg();

        assert_eq!(raw_seg.raw(), "foobar");
    }

    #[test]
    /// Test BaseSegment.compute_anchor_edit_info().
    fn test_parser_base_segments_compute_anchor_edit_info() {
        let raw_segs = raw_segments();
        let tables = Tables::default();

        // Construct a fix buffer, intentionally with:
        // - one duplicate.
        // - two different incompatible fixes on the same segment.
        let fixes = vec![
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None)],
                None,
            ),
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None)],
                None,
            ),
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("b".to_string()), None)],
                None,
            ),
        ];

        let anchor_edit_info = raw_segs[0].compute_anchor_edit_info(&fixes);

        // Check the target segment is the only key we have.
        assert_eq!(anchor_edit_info.keys().collect::<Vec<_>>(), vec![&raw_segs[0].id()]);

        let anchor_info = anchor_edit_info.get(&raw_segs[0].id()).unwrap();

        // Check that the duplicate as been deduplicated i.e. this isn't 3.
        assert_eq!(anchor_info.replace, 2);

        // Check the fixes themselves.
        //   Note: There's no duplicated first fix.
        assert_eq!(
            anchor_info.fixes[0],
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None)],
                None,
            )
        );
        assert_eq!(
            anchor_info.fixes[1],
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("b".to_string()), None)],
                None,
            )
        );

        // Check the first replace
        assert_eq!(
            anchor_info.first_replace,
            Some(LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None)],
                None,
            ))
        );
    }

    /// Test the .is_type() method.
    #[test]
    fn test_parser_base_segments_type() {
        let args = UnlexableSegmentNewArgs { expected: None };
        let segment = UnlexableSegment::create("", PositionMarker::default().into(), args);

        assert!(segment.is_type(SyntaxKind::Unlexable));
        assert!(!segment.is_type(SyntaxKind::Whitespace));
    }
}
