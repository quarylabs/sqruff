pub mod bracketed;
pub mod file;
pub mod fix;
pub mod from;
pub mod generator;
pub mod join;
pub mod meta;
pub mod object_reference;
pub mod select;
pub mod test_functions;

use std::cell::{Cell, OnceCell};
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash, Hasher};
use std::rc::Rc;

use hashbrown::{DefaultHashBuilder, HashMap, HashSet};
use itertools::enumerate;
use smol_str::SmolStr;

use crate::dialects::init::DialectKind;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::lint_fix::LintFix;
use crate::parser::markers::PositionMarker;
use crate::parser::segments::fix::{FixPatch, SourceFix};
use crate::parser::segments::object_reference::{ObjectReferenceKind, ObjectReferenceSegment};
use crate::segments::AnchorEditInfo;
use crate::templaters::TemplatedFile;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TextExtent {
    newline_count: usize,
    last_line_width: usize,
}

impl TextExtent {
    fn from_raw(raw: &str) -> Self {
        let (next_line_no, next_line_pos) = PositionMarker::infer_next_position(raw, 1, 1);
        Self {
            newline_count: next_line_no - 1,
            last_line_width: next_line_pos - 1,
        }
    }

    fn append(self, other: Self) -> Self {
        if other.newline_count == 0 {
            Self {
                newline_count: self.newline_count,
                last_line_width: self.last_line_width + other.last_line_width,
            }
        } else {
            Self {
                newline_count: self.newline_count + other.newline_count,
                last_line_width: other.last_line_width,
            }
        }
    }

    fn from_children(children: &[ErasedSegment]) -> Self {
        children.iter().fold(Self::default(), |extent, child| {
            extent.append(child.text_extent())
        })
    }

    fn advance(self, line_no: usize, line_pos: usize) -> (usize, usize) {
        if self.newline_count == 0 {
            (line_no, line_pos + self.last_line_width)
        } else {
            (line_no + self.newline_count, self.last_line_width + 1)
        }
    }
}

fn segment_id_filter(id: u32) -> u64 {
    1_u64 << (id & 63)
}

fn once_cell_with<T>(value: T) -> OnceCell<T> {
    let cell = OnceCell::new();
    let _ = cell.set(value);
    cell
}

pub struct SegmentBuilder {
    node_or_token: NodeOrToken,
}

impl SegmentBuilder {
    pub fn whitespace(id: u32, raw: &str) -> ErasedSegment {
        SegmentBuilder::token(id, raw, SyntaxKind::Whitespace).finish()
    }

    pub fn newline(id: u32, raw: &str) -> ErasedSegment {
        SegmentBuilder::token(id, raw, SyntaxKind::Newline).finish()
    }

    pub fn keyword(id: u32, raw: &str) -> ErasedSegment {
        SegmentBuilder::token(id, raw, SyntaxKind::Keyword).finish()
    }

    pub fn comma(id: u32) -> ErasedSegment {
        SegmentBuilder::token(id, ",", SyntaxKind::Comma).finish()
    }

    pub fn symbol(id: u32, raw: &str) -> ErasedSegment {
        SegmentBuilder::token(id, raw, SyntaxKind::Symbol).finish()
    }

    pub fn node(
        id: u32,
        syntax_kind: SyntaxKind,
        dialect: DialectKind,
        segments: Vec<ErasedSegment>,
    ) -> Self {
        SegmentBuilder {
            node_or_token: NodeOrToken {
                id,
                syntax_kind,
                class_types: class_types(syntax_kind),
                position_marker: None,
                code_idx: OnceCell::new(),
                kind: NodeOrTokenKind::Node(NodeData {
                    dialect,
                    segments,
                    raw: Default::default(),
                    source_fixes: vec![],
                    descendant_type_set: Default::default(),
                    raw_segments_with_ancestors: Default::default(),
                }),
                hash: OnceCell::new(),
                text_extent: OnceCell::new(),
                subtree_anchor_filter: OnceCell::new(),
                position_basis_start: OnceCell::new(),
            },
        }
    }

    pub fn token(id: u32, raw: &str, syntax_kind: SyntaxKind) -> Self {
        SegmentBuilder {
            node_or_token: NodeOrToken {
                id,
                syntax_kind,
                code_idx: OnceCell::new(),
                class_types: class_types(syntax_kind),
                position_marker: None,
                kind: NodeOrTokenKind::Token(TokenData { raw: raw.into() }),
                hash: OnceCell::new(),
                text_extent: OnceCell::new(),
                subtree_anchor_filter: OnceCell::new(),
                position_basis_start: OnceCell::new(),
            },
        }
    }

    pub fn position_from_segments(mut self) -> Self {
        let segments = match &self.node_or_token.kind {
            NodeOrTokenKind::Node(node) => &node.segments[..],
            NodeOrTokenKind::Token(_) => &[],
        };

        self.node_or_token.position_marker = pos_marker(segments).into();
        self
    }

    pub fn with_position(mut self, position: PositionMarker) -> Self {
        self.node_or_token.position_marker = Some(position);
        self
    }

    pub fn with_source_fixes(mut self, source_fixes: Vec<SourceFix>) -> Self {
        if let NodeOrTokenKind::Node(ref mut node) = self.node_or_token.kind {
            node.source_fixes = source_fixes;
        }
        self
    }

    pub fn finish(self) -> ErasedSegment {
        ErasedSegment {
            value: Rc::new(self.node_or_token),
        }
    }
}

#[derive(Debug, Default)]
pub struct Tables {
    counter: Cell<u32>,
}

impl Tables {
    pub fn next_id(&self) -> u32 {
        let id = self.counter.get();
        self.counter.set(id + 1);
        id
    }
}

#[derive(Debug, Clone)]
pub struct ErasedSegment {
    pub(crate) value: Rc<NodeOrToken>,
}

impl Hash for ErasedSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_value().hash(state);
    }
}

impl Eq for ErasedSegment {}

impl ErasedSegment {
    pub fn raw(&self) -> &SmolStr {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.raw.get_or_init(|| {
                SmolStr::from_iter(self.segments().iter().map(|segment| segment.raw().as_str()))
            }),
            NodeOrTokenKind::Token(token) => &token.raw,
        }
    }

    fn text_extent(&self) -> TextExtent {
        *self
            .value
            .text_extent
            .get_or_init(|| match &self.value.kind {
                NodeOrTokenKind::Node(node) => TextExtent::from_children(&node.segments),
                NodeOrTokenKind::Token(token) => TextExtent::from_raw(&token.raw),
            })
    }

    fn subtree_anchor_filter(&self) -> u64 {
        *self.value.subtree_anchor_filter.get_or_init(|| {
            segment_id_filter(self.id())
                | self
                    .segments()
                    .iter()
                    .fold(0, |filter, child| filter | child.subtree_anchor_filter())
        })
    }

    fn position_basis_start(&self) -> Option<PositionMarker> {
        self.value
            .position_basis_start
            .get_or_init(|| {
                let pos = self.get_position_marker()?;
                if self.segments().is_empty() {
                    return Some(pos.start_point_marker());
                }

                self.segments()
                    .iter()
                    .find_map(ErasedSegment::position_basis_start)
            })
            .clone()
    }

    pub fn segments(&self) -> &[ErasedSegment] {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => &node.segments,
            NodeOrTokenKind::Token(_) => &[],
        }
    }

    pub fn get_type(&self) -> SyntaxKind {
        self.value.syntax_kind
    }

    pub fn is_type(&self, kind: SyntaxKind) -> bool {
        self.get_type() == kind
    }

    pub fn is_meta(&self) -> bool {
        matches!(
            self.value.syntax_kind,
            SyntaxKind::Indent | SyntaxKind::Implicit | SyntaxKind::Dedent | SyntaxKind::EndOfFile
        )
    }

    pub fn is_code(&self) -> bool {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.segments.iter().any(|s| s.is_code()),
            NodeOrTokenKind::Token(_) => {
                !self.is_comment() && !self.is_whitespace() && !self.is_meta()
            }
        }
    }

    pub fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        self.recursive_crawl_all(false)
            .into_iter()
            .filter(|it| it.segments().is_empty())
            .collect()
    }

    #[cfg(feature = "stringify")]
    pub fn stringify(&self, code_only: bool) -> String {
        serde_yaml::to_string(&self.to_serialised(code_only, true)).unwrap()
    }

    pub fn child(&self, seg_types: &SyntaxSet) -> Option<ErasedSegment> {
        self.segments()
            .iter()
            .find(|seg| seg_types.contains(seg.get_type()))
            .cloned()
    }

    pub fn recursive_crawl(
        &self,
        types: &SyntaxSet,
        recurse_into: bool,
        no_recursive_types: &SyntaxSet,
        allow_self: bool,
    ) -> Vec<ErasedSegment> {
        let mut acc = Vec::new();

        let matches = allow_self && self.class_types().intersects(types);
        if matches {
            acc.push(self.clone());
        }

        if !self.descendant_type_set().intersects(types) {
            return acc;
        }

        if recurse_into || !matches {
            for seg in self.segments() {
                if no_recursive_types.is_empty() || !no_recursive_types.contains(seg.get_type()) {
                    let segments =
                        seg.recursive_crawl(types, recurse_into, no_recursive_types, true);
                    acc.extend(segments);
                }
            }
        }

        acc
    }
}

impl ErasedSegment {
    #[track_caller]
    pub fn new(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => {
                let mut builder = SegmentBuilder::node(
                    self.value.id,
                    self.value.syntax_kind,
                    node.dialect,
                    segments,
                )
                .with_position(self.get_position_marker().unwrap().clone());
                // Preserve source_fixes during tree rebuilds
                if !node.source_fixes.is_empty() {
                    builder = builder.with_source_fixes(node.source_fixes.clone());
                }
                builder.finish()
            }
            NodeOrTokenKind::Token(_) => self.deep_clone(),
        }
    }

    pub fn indent_val(&self) -> i8 {
        self.value.syntax_kind.indent_val()
    }

    pub fn can_start_end_non_code(&self) -> bool {
        matches!(
            self.value.syntax_kind,
            SyntaxKind::File | SyntaxKind::Unparsable
        )
    }

    pub(crate) fn dialect(&self) -> DialectKind {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.dialect,
            NodeOrTokenKind::Token(_) => todo!(),
        }
    }

    pub fn get_start_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc(),
            None => unreachable!("{self:?} has no PositionMarker"),
        }
    }

    pub fn get_end_loc(&self) -> (usize, usize) {
        match self.get_position_marker() {
            Some(pos_marker) => pos_marker.working_loc_after(self.raw()),
            None => {
                unreachable!("{self:?} has no PositionMarker")
            }
        }
    }

    pub fn is_templated(&self) -> bool {
        if let Some(pos_marker) = self.get_position_marker() {
            pos_marker.source_slice.start != pos_marker.source_slice.end && !pos_marker.is_literal()
        } else {
            panic!("PosMarker must be set");
        }
    }

    pub fn iter_segments(&self, expanding: &SyntaxSet, pass_through: bool) -> Vec<ErasedSegment> {
        let capacity = if expanding.is_empty() {
            self.segments().len()
        } else {
            0
        };
        let mut result = Vec::with_capacity(capacity);
        for segment in self.segments() {
            if expanding.contains(segment.get_type()) {
                let expanding = if pass_through {
                    expanding
                } else {
                    &SyntaxSet::EMPTY
                };
                result.append(&mut segment.iter_segments(expanding, false));
            } else {
                result.push(segment.clone());
            }
        }
        result
    }

    pub(crate) fn code_indices(&self) -> Rc<Vec<usize>> {
        self.value
            .code_idx
            .get_or_init(|| {
                Rc::from(
                    self.segments()
                        .iter()
                        .enumerate()
                        .filter(|(_, seg)| seg.is_code())
                        .map(|(idx, _)| idx)
                        .collect::<Vec<_>>(),
                )
            })
            .clone()
    }

    pub fn children(
        &self,
        seg_types: &'static SyntaxSet,
    ) -> impl Iterator<Item = &ErasedSegment> + '_ {
        self.segments()
            .iter()
            .filter(move |seg| seg_types.contains(seg.get_type()))
    }

    pub fn iter_patches(&self, templated_file: &TemplatedFile) -> Vec<FixPatch> {
        let mut acc = Vec::new();

        let templated_raw = &templated_file.templated_str.as_ref().unwrap()
            [self.get_position_marker().unwrap().templated_slice.clone()];

        // Always collect source fixes from this segment first
        acc.extend(self.iter_source_fix_patches(templated_file));

        // Check if any descendants have source_fixes
        let has_descendant_source_fixes = self
            .recursive_crawl_all(false)
            .iter()
            .any(|s| !s.get_source_fixes().is_empty());

        if self.raw() == templated_raw {
            if has_descendant_source_fixes {
                // Tree raw hasn't changed - only source fix patches are needed.
                // Avoid generating gap patches that could span template boundaries.
                // This matches SQLFluff's behavior in _iter_templated_patches.
                for descendant in self.recursive_crawl_all(false).into_iter().skip(1) {
                    acc.extend(descendant.iter_source_fix_patches(templated_file));
                }
            }
            return acc;
        }

        if self.get_position_marker().is_none() {
            return Vec::new();
        }

        let pos_marker = self.get_position_marker().unwrap();
        if pos_marker.is_literal() && !has_descendant_source_fixes {
            acc.extend(self.iter_source_fix_patches(templated_file));
            acc.push(FixPatch::new(
                pos_marker.templated_slice.clone(),
                self.raw().clone(),
                // SyntaxKind::Literal.into(),
                pos_marker.source_slice.clone(),
                templated_file.templated_str.as_ref().unwrap()[pos_marker.templated_slice.clone()]
                    .to_string(),
                templated_file.source_str[pos_marker.source_slice.clone()].to_string(),
            ));
        } else if self.segments().is_empty() {
            return acc;
        } else {
            let mut segments = self.segments();

            while !segments.is_empty()
                && matches!(
                    segments.last().unwrap().get_type(),
                    SyntaxKind::EndOfFile
                        | SyntaxKind::Indent
                        | SyntaxKind::Dedent
                        | SyntaxKind::Implicit
                )
            {
                segments = &segments[..segments.len() - 1];
            }

            let pos = self.get_position_marker().unwrap();
            let mut source_idx = pos.source_slice.start;
            let mut templated_idx = pos.templated_slice.start;
            let mut insert_buff = String::new();

            for segment in segments {
                let pos_marker = segment.get_position_marker().unwrap();
                if !segment.raw().is_empty() && pos_marker.is_point() {
                    insert_buff.push_str(segment.raw().as_ref());
                    continue;
                }

                let start_diff = pos_marker.templated_slice.start - templated_idx;

                if start_diff > 0 || !insert_buff.is_empty() {
                    let fixed_raw = std::mem::take(&mut insert_buff);
                    let raw_segments = segment.get_raw_segments();
                    let first_segment_pos = raw_segments[0].get_position_marker().unwrap();

                    // The slices must never go backwards so the end of the slice
                    // must be >= the start. This can happen when source positions
                    // are non-monotonic due to template expansion.
                    acc.push(FixPatch::new(
                        templated_idx..first_segment_pos.templated_slice.start.max(templated_idx),
                        fixed_raw.into(),
                        source_idx..first_segment_pos.source_slice.start.max(source_idx),
                        String::new(),
                        String::new(),
                    ));
                }

                acc.extend(segment.iter_patches(templated_file));

                source_idx = pos_marker.source_slice.end;
                templated_idx = pos_marker.templated_slice.end;
            }

            let end_diff = pos.templated_slice.end - templated_idx;
            if end_diff != 0 || !insert_buff.is_empty() {
                let source_slice = source_idx..pos.source_slice.end;
                let templated_slice = templated_idx..pos.templated_slice.end;

                let templated_str = templated_file.templated_str.as_ref().unwrap()
                    [templated_slice.clone()]
                .to_owned();
                let source_str = templated_file.source_str[source_slice.clone()].to_owned();

                acc.push(FixPatch::new(
                    templated_slice,
                    insert_buff.into(),
                    source_slice,
                    templated_str,
                    source_str,
                ));
            }
        }

        acc
    }

    pub fn descendant_type_set(&self) -> &SyntaxSet {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.descendant_type_set.get_or_init(|| {
                self.segments()
                    .iter()
                    .flat_map(|segment| {
                        segment
                            .descendant_type_set()
                            .clone()
                            .union(segment.class_types())
                    })
                    .collect()
            }),
            NodeOrTokenKind::Token(_) => const { &SyntaxSet::EMPTY },
        }
    }

    pub fn is_comment(&self) -> bool {
        matches!(
            self.value.syntax_kind,
            SyntaxKind::Comment
                | SyntaxKind::InlineComment
                | SyntaxKind::BlockComment
                | SyntaxKind::NotebookStart
        )
    }

    pub fn is_whitespace(&self) -> bool {
        matches!(
            self.value.syntax_kind,
            SyntaxKind::Whitespace | SyntaxKind::Newline
        )
    }

    pub fn is_indent(&self) -> bool {
        matches!(
            self.value.syntax_kind,
            SyntaxKind::Indent | SyntaxKind::Implicit | SyntaxKind::Dedent
        )
    }

    pub fn get_position_marker(&self) -> Option<&PositionMarker> {
        self.value.position_marker.as_ref()
    }

    pub(crate) fn iter_source_fix_patches(&self, templated_file: &TemplatedFile) -> Vec<FixPatch> {
        let source_fixes = self.get_source_fixes();
        let mut patches = Vec::with_capacity(source_fixes.len());

        for source_fix in &source_fixes {
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

    pub fn id(&self) -> u32 {
        self.value.id
    }

    /// Return any source fixes as list.
    pub fn get_source_fixes(&self) -> Vec<SourceFix> {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.source_fixes.clone(),
            NodeOrTokenKind::Token(_) => Vec::new(),
        }
    }

    /// Return all source fixes from this segment and all its descendants.
    pub fn get_all_source_fixes(&self) -> Vec<SourceFix> {
        let mut fixes = self.get_source_fixes();
        for segment in self.segments() {
            fixes.extend(segment.get_all_source_fixes());
        }
        fixes
    }

    pub fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        match &self.value.kind {
            NodeOrTokenKind::Node(_node) => {
                todo!()
            }
            NodeOrTokenKind::Token(token) => {
                let raw = raw.as_deref().unwrap_or(token.raw.as_ref());
                SegmentBuilder::token(id, raw, self.value.syntax_kind)
                    .with_position(self.get_position_marker().unwrap().clone())
                    .finish()
            }
        }
    }

    pub fn class_types(&self) -> &SyntaxSet {
        &self.value.class_types
    }

    pub(crate) fn first_non_whitespace_segment_raw_upper(&self) -> Option<String> {
        for seg in self.get_raw_segments() {
            if !seg.raw().is_empty() {
                return Some(seg.raw().to_uppercase());
            }
        }
        None
    }

    pub fn is(&self, other: &ErasedSegment) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }

    pub fn addr(&self) -> usize {
        Rc::as_ptr(&self.value).addr()
    }

    pub fn direct_descendant_type_set(&self) -> SyntaxSet {
        self.segments()
            .iter()
            .fold(SyntaxSet::EMPTY, |set, it| set.union(it.class_types()))
    }

    pub fn is_keyword(&self, p0: &str) -> bool {
        self.is_type(SyntaxKind::Keyword) && self.raw().eq_ignore_ascii_case(p0)
    }

    pub fn hash_value(&self) -> u64 {
        *self.value.hash.get_or_init(|| {
            let mut hasher = DefaultHashBuilder::default().build_hasher();
            self.get_type().hash(&mut hasher);
            self.raw().hash(&mut hasher);

            if let Some(marker) = &self.get_position_marker() {
                marker.source_position().hash(&mut hasher);
            } else {
                None::<usize>.hash(&mut hasher);
            }

            hasher.finish()
        })
    }

    pub fn deep_clone(&self) -> Self {
        Self {
            value: Rc::new(self.value.as_ref().clone()),
        }
    }

    #[track_caller]
    pub(crate) fn get_mut(&mut self) -> &mut NodeOrToken {
        Rc::get_mut(&mut self.value).unwrap()
    }

    #[track_caller]
    pub(crate) fn make_mut(&mut self) -> &mut NodeOrToken {
        Rc::make_mut(&mut self.value)
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
        let mut result = Vec::with_capacity(self.segments().len() + 1);

        if reverse {
            for seg in self.segments().iter().rev() {
                result.append(&mut seg.recursive_crawl_all(reverse));
            }
            result.push(self.clone());
        } else {
            result.push(self.clone());
            for seg in self.segments() {
                result.append(&mut seg.recursive_crawl_all(reverse));
            }
        }

        result
    }

    pub fn raw_segments_with_ancestors(&self) -> &[(ErasedSegment, Vec<PathStep>)] {
        match &self.value.kind {
            NodeOrTokenKind::Node(node) => node.raw_segments_with_ancestors.get_or_init(|| {
                let mut buffer: Vec<(ErasedSegment, Vec<PathStep>)> =
                    Vec::with_capacity(self.segments().len());
                let code_idxs = self.code_indices();

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
                            seg.raw_segments_with_ancestors()
                                .iter()
                                .map(|(raw_seg, stack)| {
                                    let mut new_step = new_step.clone();
                                    new_step.extend_from_slice(stack);
                                    (raw_seg.clone(), new_step)
                                });

                        buffer.extend(extended);
                    }
                }

                buffer
            }),
            NodeOrTokenKind::Token(_) => &[],
        }
    }

    pub fn path_to(&self, other: &ErasedSegment) -> Vec<PathStep> {
        let midpoint = other;

        for (idx, seg) in enumerate(self.segments()) {
            let mut steps = vec![PathStep {
                segment: self.clone(),
                idx,
                len: self.segments().len(),
                code_idxs: self.code_indices(),
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
        fixes: &mut HashMap<u32, AnchorEditInfo>,
    ) -> (ErasedSegment, Vec<ErasedSegment>, Vec<ErasedSegment>) {
        if fixes.is_empty() || self.segments().is_empty() {
            return (self.clone(), Vec::new(), Vec::new());
        }

        let pending_filter = fixes
            .keys()
            .fold(0_u64, |filter, id| filter | segment_id_filter(*id));
        if self.subtree_anchor_filter() & pending_filter == 0 {
            return (self.clone(), Vec::new(), Vec::new());
        }

        let mut applicable_fixes = HashMap::new();
        for segment in self.recursive_crawl_all(false).into_iter().skip(1) {
            if let Some(anchor_info) = fixes.remove(&segment.id()) {
                applicable_fixes.insert(segment.id(), anchor_info);
            }
        }

        if applicable_fixes.is_empty() {
            return (self.clone(), Vec::new(), Vec::new());
        }

        (
            self.apply_fixes_v2(applicable_fixes),
            Vec::new(),
            Vec::new(),
        )
    }

    pub fn apply_fixes_v2(&self, fixes: HashMap<u32, AnchorEditInfo>) -> ErasedSegment {
        if fixes.is_empty() || self.segments().is_empty() {
            return self.clone();
        }

        let program = FixProgram::compile(fixes);
        let frontier = locate_frontier(self, &program.plans, program.pending_filter);
        if frontier.is_empty() {
            return self.clone();
        }

        let parent_pos = self
            .get_position_marker()
            .cloned()
            .expect("segments with children must have a position marker");
        let mut context = PrepareContext::new(frontier);
        let prepared = prepare_segment(
            self.clone(),
            None,
            PrepareOrigin::OriginalTree,
            &mut context,
        );

        freeze_prepared_child(&prepared, parent_pos, &context.arena)
    }
}

#[cfg(any(test, feature = "serde"))]
pub mod serde {
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Serialize};

    use crate::parser::segments::ErasedSegment;

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
        pub fn sinlge(key: String, value: String) -> Self {
            Self(key, SerialisedSegmentValue::Single(value))
        }

        pub fn nested(key: String, segments: Vec<TupleSerialisedSegment>) -> Self {
            Self(key, SerialisedSegmentValue::Nested(segments))
        }
    }

    impl ErasedSegment {
        pub fn to_serialised(&self, code_only: bool, show_raw: bool) -> TupleSerialisedSegment {
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
                    .map(|seg| seg.to_serialised(code_only, show_raw))
                    .collect::<Vec<_>>();

                TupleSerialisedSegment::nested(self.get_type().as_str().to_string(), segments)
            } else {
                let segments = self
                    .segments()
                    .iter()
                    .map(|seg| seg.to_serialised(code_only, show_raw))
                    .collect::<Vec<_>>();

                TupleSerialisedSegment::nested(self.get_type().as_str().to_string(), segments)
            }
        }
    }
}

impl PartialEq for ErasedSegment {
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

#[derive(Debug)]
struct FixProgram {
    plans: HashMap<u32, AnchorPlan>,
    pending_filter: u64,
}

impl FixProgram {
    fn compile(fixes: HashMap<u32, AnchorEditInfo>) -> Self {
        let mut plans = HashMap::with_capacity(fixes.len());
        let mut pending_filter = 0;

        for (anchor_id, anchor_info) in fixes {
            pending_filter |= segment_id_filter(anchor_id);
            plans.insert(anchor_id, AnchorPlan::compile(anchor_info));
        }

        Self {
            plans,
            pending_filter,
        }
    }
}

#[derive(Debug, Clone)]
struct AnchorPlan {
    steps: Vec<Step>,
}

impl AnchorPlan {
    fn compile(mut anchor_info: AnchorEditInfo) -> Self {
        if anchor_info.fixes.len() == 2
            && matches!(anchor_info.fixes[0], LintFix::CreateAfter { .. })
        {
            anchor_info.fixes.reverse();
        }

        let fixes_count = anchor_info.fixes.len();
        let mut steps = Vec::with_capacity(fixes_count * 2);

        for fix in anchor_info.fixes {
            match fix {
                LintFix::CreateAfter { edit, .. } => {
                    if fixes_count == 1 {
                        steps.push(Step::KeepAnchor);
                    }
                    steps.push(Step::Insert(edit));
                }
                LintFix::CreateBefore { edit, .. } => {
                    steps.push(Step::Insert(edit));
                    steps.push(Step::KeepAnchor);
                }
                LintFix::Replace { anchor, edit, .. } => {
                    let seed_match_idx = edit
                        .iter()
                        .position(|segment| segment.raw() == anchor.raw());
                    steps.push(Step::Replace {
                        edit,
                        seed_match_idx,
                    });
                }
                LintFix::Delete { .. } => steps.push(Step::DeleteAnchor),
            }
        }

        Self { steps }
    }

    fn keeps_anchor(&self) -> bool {
        self.steps
            .iter()
            .any(|step| matches!(step, Step::KeepAnchor))
    }

    fn emitted_len(&self) -> usize {
        self.steps
            .iter()
            .map(|step| match step {
                Step::KeepAnchor => 1,
                Step::Insert(edit) | Step::Replace { edit, .. } => edit.len(),
                Step::DeleteAnchor => 0,
            })
            .sum()
    }
}

#[derive(Debug, Clone)]
enum Step {
    KeepAnchor,
    Insert(Vec<ErasedSegment>),
    Replace {
        edit: Vec<ErasedSegment>,
        seed_match_idx: Option<usize>,
    },
    DeleteAnchor,
}

type ArenaIdx = usize;

#[derive(Debug)]
struct Frontier {
    affected_nodes: HashSet<u32>,
    parent_patches: HashMap<u32, ParentPatch>,
}

impl Frontier {
    fn is_empty(&self) -> bool {
        self.affected_nodes.is_empty() && self.parent_patches.is_empty()
    }
}

#[derive(Debug, Default)]
struct ParentPatch {
    edits: Vec<ChildPatch>,
    final_len_hint: usize,
}

#[derive(Debug, Clone)]
struct ChildPatch {
    child_idx: usize,
    anchor_id: u32,
    plan: AnchorPlan,
}

#[derive(Debug, Clone)]
struct ChildSummary {
    extent: TextExtent,
    basis_start: Option<PositionMarker>,
}

#[derive(Debug)]
struct PreparedChild {
    base: ErasedSegment,
    built: Option<ArenaIdx>,
    seed: Option<PositionMarker>,
    summary: ChildSummary,
    content_changed: bool,
}

impl PreparedChild {
    fn reuse(base: ErasedSegment, seed: Option<PositionMarker>) -> Self {
        let extent = base.text_extent();
        let summary = child_summary(&seed, &extent, &base);
        Self {
            base,
            built: None,
            seed: seed.clone(),
            summary,
            content_changed: false,
        }
    }

    fn position_seed(&self) -> Option<&PositionMarker> {
        self.seed
            .as_ref()
            .or_else(|| self.base.get_position_marker())
    }
}

#[derive(Debug)]
struct BuildNode {
    base: ErasedSegment,
    children: Vec<PreparedChild>,
    content_changed: bool,
    summary: ChildSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreezeMode {
    PositionOnly,
    ContentChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepareOrigin {
    OriginalTree,
    EmittedEdit,
}

#[derive(Debug)]
struct PrepareContext {
    frontier: Frontier,
    arena: Vec<BuildNode>,
}

impl PrepareContext {
    fn new(frontier: Frontier) -> Self {
        Self {
            frontier,
            arena: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct FrontierFrame {
    segment: ErasedSegment,
    next_child_idx: usize,
    any_match: bool,
}

fn locate_frontier(
    root: &ErasedSegment,
    plans: &HashMap<u32, AnchorPlan>,
    pending_filter: u64,
) -> Frontier {
    let mut frontier = Frontier {
        affected_nodes: HashSet::new(),
        parent_patches: HashMap::new(),
    };

    if root.subtree_anchor_filter() & pending_filter == 0 {
        return frontier;
    }

    let mut stack = vec![FrontierFrame {
        segment: root.clone(),
        next_child_idx: 0,
        any_match: false,
    }];

    while let Some(frame) = stack.last_mut() {
        if frame.next_child_idx == frame.segment.segments().len() {
            let finished = stack.pop().unwrap();
            if finished.any_match {
                frontier.affected_nodes.insert(finished.segment.id());
                if let Some(parent) = stack.last_mut() {
                    parent.any_match = true;
                }
            }
            continue;
        }

        let child_idx = frame.next_child_idx;
        frame.next_child_idx += 1;
        let child = frame.segment.segments()[child_idx].clone();

        if let Some(plan) = plans.get(&child.id()) {
            let parent_patch = frontier
                .parent_patches
                .entry(frame.segment.id())
                .or_insert_with(|| ParentPatch {
                    edits: Vec::new(),
                    final_len_hint: frame.segment.segments().len(),
                });
            parent_patch.final_len_hint = parent_patch.final_len_hint - 1 + plan.emitted_len();
            parent_patch.edits.push(ChildPatch {
                child_idx,
                anchor_id: child.id(),
                plan: plan.clone(),
            });
            frame.any_match = true;

            if !plan.keeps_anchor() {
                continue;
            }
        }

        if child.segments().is_empty() || child.subtree_anchor_filter() & pending_filter == 0 {
            continue;
        }

        stack.push(FrontierFrame {
            segment: child,
            next_child_idx: 0,
            any_match: false,
        });
    }

    frontier
}

fn child_summary(
    seed: &Option<PositionMarker>,
    extent: &TextExtent,
    base: &ErasedSegment,
) -> ChildSummary {
    ChildSummary {
        extent: *extent,
        basis_start: seed
            .as_ref()
            .map(PositionMarker::start_point_marker)
            .or_else(|| base.position_basis_start()),
    }
}

fn prepare_anchor_plan(
    anchor: &ErasedSegment,
    plan: AnchorPlan,
    context: &mut PrepareContext,
) -> Vec<PreparedChild> {
    let mut emitted = Vec::new();

    for step in plan.steps {
        match step {
            Step::KeepAnchor => emitted.push(prepare_segment(
                anchor.clone(),
                None,
                PrepareOrigin::OriginalTree,
                context,
            )),
            Step::Insert(edit) => {
                emitted.extend(edit.into_iter().map(|segment| {
                    prepare_segment(segment, None, PrepareOrigin::EmittedEdit, context)
                }));
            }
            Step::Replace {
                edit,
                seed_match_idx,
            } => {
                for (idx, segment) in edit.into_iter().enumerate() {
                    emitted.push(prepare_segment(
                        segment,
                        (seed_match_idx == Some(idx))
                            .then(|| anchor.get_position_marker().cloned())
                            .flatten(),
                        PrepareOrigin::EmittedEdit,
                        context,
                    ));
                }
            }
            Step::DeleteAnchor => {}
        }
    }

    emitted
}

fn prepare_children(
    parent: &ErasedSegment,
    patch: Option<ParentPatch>,
    context: &mut PrepareContext,
) -> (Vec<PreparedChild>, bool) {
    let capacity = patch
        .as_ref()
        .map(|patch| patch.final_len_hint.max(parent.segments().len()))
        .unwrap_or_else(|| parent.segments().len());
    let mut children = Vec::with_capacity(capacity);
    let mut content_changed = false;
    let mut patch_iter = patch
        .map(|patch| patch.edits.into_iter())
        .into_iter()
        .flatten()
        .peekable();

    for (idx, child) in parent.segments().iter().enumerate() {
        if patch_iter
            .peek()
            .is_some_and(|patch| patch.child_idx == idx && patch.anchor_id == child.id())
        {
            let patch = patch_iter.next().unwrap();
            content_changed = true;
            children.extend(prepare_anchor_plan(child, patch.plan, context));
            continue;
        }

        let prepared = prepare_segment(child.clone(), None, PrepareOrigin::OriginalTree, context);
        content_changed |= prepared.content_changed;
        children.push(prepared);
    }

    debug_assert!(patch_iter.next().is_none());

    (children, content_changed)
}

fn prepare_segment(
    segment: ErasedSegment,
    seed: Option<PositionMarker>,
    origin: PrepareOrigin,
    context: &mut PrepareContext,
) -> PreparedChild {
    if segment.segments().is_empty() {
        return PreparedChild::reuse(segment, seed);
    }

    let (is_affected, patch) = match origin {
        PrepareOrigin::OriginalTree => (
            context.frontier.affected_nodes.contains(&segment.id()),
            context.frontier.parent_patches.remove(&segment.id()),
        ),
        PrepareOrigin::EmittedEdit => (false, None),
    };

    if !is_affected && patch.is_none() {
        return PreparedChild::reuse(segment, seed);
    }

    let (children, content_changed) = prepare_children(&segment, patch, context);
    let extent = children
        .iter()
        .fold(TextExtent::default(), |extent, child| {
            extent.append(child.summary.extent)
        });
    let summary = child_summary(&seed, &extent, &segment);

    let built_idx = context.arena.len();
    context.arena.push(BuildNode {
        base: segment.clone(),
        children,
        content_changed,
        summary: summary.clone(),
    });

    PreparedChild {
        base: segment,
        built: Some(built_idx),
        seed,
        summary,
        content_changed,
    }
}

fn compute_next_known_start_prepared(children: &[PreparedChild]) -> Vec<Option<PositionMarker>> {
    let mut next_known_start = vec![None; children.len()];
    let mut right_anchor = None;

    for idx in (0..children.len()).rev() {
        next_known_start[idx] = right_anchor.clone();
        if let Some(pos) = children[idx].summary.basis_start.clone() {
            right_anchor = Some(pos);
        }
    }

    next_known_start
}

fn compute_next_known_start_segments(children: &[ErasedSegment]) -> Vec<Option<PositionMarker>> {
    let mut next_known_start = vec![None; children.len()];
    let mut right_anchor = None;

    for idx in (0..children.len()).rev() {
        next_known_start[idx] = right_anchor.clone();
        if let Some(pos) = children[idx].position_basis_start() {
            right_anchor = Some(pos);
        }
    }

    next_known_start
}

fn infer_position(
    basis: Option<&PositionMarker>,
    left_anchor: Option<&PositionMarker>,
    right_anchor: Option<&PositionMarker>,
    line_no: usize,
    line_pos: usize,
) -> PositionMarker {
    basis
        .cloned()
        .unwrap_or_else(|| {
            if let Some((left_anchor, right_anchor)) = left_anchor
                .zip(right_anchor)
                .filter(|(left_anchor, right_anchor)| left_anchor != right_anchor)
            {
                PositionMarker::from_points(left_anchor, right_anchor)
            } else if let Some(left_anchor) = left_anchor {
                left_anchor.clone()
            } else if let Some(right_anchor) = right_anchor {
                right_anchor.clone()
            } else {
                unreachable!("Unable to position new segment")
            }
        })
        .with_working_position(line_no, line_pos)
}

fn freeze_prepared_child(
    child: &PreparedChild,
    pos: PositionMarker,
    arena: &[BuildNode],
) -> ErasedSegment {
    let Some(built_idx) = child.built else {
        return position_only_freeze_base(&child.base, pos);
    };

    let node = &arena[built_idx];
    let next_known_start = compute_next_known_start_prepared(&node.children);
    let (mut line_no, mut line_pos) = pos.working_loc();
    let mut left_anchor = Some(pos.start_point_marker());
    let mut finalized_children = Vec::with_capacity(node.children.len());
    let base_children = node.base.segments();
    let mut all_same_children = base_children.len() == node.children.len();

    for (idx, prepared_child) in node.children.iter().enumerate() {
        let child_pos = infer_position(
            prepared_child.position_seed(),
            left_anchor.as_ref(),
            next_known_start[idx].as_ref(),
            line_no,
            line_pos,
        );
        let finalized_child = freeze_prepared_child(prepared_child, child_pos.clone(), arena);
        if all_same_children && !base_children[idx].is(&finalized_child) {
            all_same_children = false;
        }

        (line_no, line_pos) = prepared_child.summary.extent.advance(line_no, line_pos);
        left_anchor = Some(child_pos.end_point_marker());
        finalized_children.push(finalized_child);
    }

    if !node.content_changed && node.base.get_position_marker() == Some(&pos) && all_same_children {
        return node.base.clone();
    }

    freeze_with_children(
        &node.base,
        finalized_children,
        pos,
        if node.content_changed {
            FreezeMode::ContentChanged
        } else {
            FreezeMode::PositionOnly
        },
        node.summary.extent,
    )
}

fn position_only_freeze_base(base: &ErasedSegment, pos: PositionMarker) -> ErasedSegment {
    if base.get_position_marker() == Some(&pos) {
        return base.clone();
    }

    if base.segments().is_empty() {
        let mut segment = base.deep_clone();
        segment.get_mut().set_position_marker(Some(pos));
        return segment;
    }

    let finalized_children = position_segments(base.segments(), &pos);
    freeze_with_children(
        base,
        finalized_children,
        pos,
        FreezeMode::PositionOnly,
        base.text_extent(),
    )
}

fn freeze_with_children(
    base: &ErasedSegment,
    children: Vec<ErasedSegment>,
    pos: PositionMarker,
    mode: FreezeMode,
    extent: TextExtent,
) -> ErasedSegment {
    let NodeOrTokenKind::Node(node) = &base.value.kind else {
        let mut segment = base.deep_clone();
        segment.get_mut().set_position_marker(Some(pos));
        return segment;
    };

    let subtree_anchor_filter = segment_id_filter(base.id())
        | children
            .iter()
            .fold(0, |filter, child| filter | child.subtree_anchor_filter());

    ErasedSegment {
        value: Rc::new(NodeOrToken {
            id: base.value.id,
            syntax_kind: base.value.syntax_kind,
            class_types: base.value.class_types.clone(),
            position_marker: Some(pos),
            kind: NodeOrTokenKind::Node(NodeData {
                dialect: node.dialect,
                segments: children,
                raw: match mode {
                    FreezeMode::PositionOnly => node.raw.clone(),
                    FreezeMode::ContentChanged => OnceCell::new(),
                },
                source_fixes: node.source_fixes.clone(),
                descendant_type_set: match mode {
                    FreezeMode::PositionOnly => node.descendant_type_set.clone(),
                    FreezeMode::ContentChanged => OnceCell::new(),
                },
                raw_segments_with_ancestors: match mode {
                    FreezeMode::PositionOnly => node.raw_segments_with_ancestors.clone(),
                    FreezeMode::ContentChanged => OnceCell::new(),
                },
            }),
            code_idx: OnceCell::new(),
            hash: OnceCell::new(),
            text_extent: match mode {
                FreezeMode::PositionOnly => base.value.text_extent.clone(),
                FreezeMode::ContentChanged => once_cell_with(extent),
            },
            subtree_anchor_filter: match mode {
                FreezeMode::PositionOnly => base.value.subtree_anchor_filter.clone(),
                FreezeMode::ContentChanged => once_cell_with(subtree_anchor_filter),
            },
            position_basis_start: OnceCell::new(),
        }),
    }
}

pub fn position_segments(
    segments: &[ErasedSegment],
    parent_pos: &PositionMarker,
) -> Vec<ErasedSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let next_known_start = compute_next_known_start_segments(segments);
    let (mut line_no, mut line_pos) = parent_pos.working_loc();
    let mut left_anchor = Some(parent_pos.start_point_marker());
    let mut finalized = Vec::with_capacity(segments.len());

    for (idx, segment) in segments.iter().enumerate() {
        let new_pos = infer_position(
            segment.get_position_marker(),
            left_anchor.as_ref(),
            next_known_start[idx].as_ref(),
            line_no,
            line_pos,
        );
        (line_no, line_pos) = segment.text_extent().advance(line_no, line_pos);
        left_anchor = Some(new_pos.end_point_marker());
        finalized.push(position_only_freeze_base(segment, new_pos));
    }

    finalized
}

#[derive(Debug, Clone)]
pub struct NodeOrToken {
    id: u32,
    syntax_kind: SyntaxKind,
    class_types: SyntaxSet,
    position_marker: Option<PositionMarker>,
    kind: NodeOrTokenKind,
    code_idx: OnceCell<Rc<Vec<usize>>>,
    hash: OnceCell<u64>,
    text_extent: OnceCell<TextExtent>,
    subtree_anchor_filter: OnceCell<u64>,
    position_basis_start: OnceCell<Option<PositionMarker>>,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum NodeOrTokenKind {
    Node(NodeData),
    Token(TokenData),
}

impl NodeOrToken {
    pub fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
        self.position_basis_start = OnceCell::new();
    }

    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }
}

#[derive(Debug, Clone)]
pub struct NodeData {
    dialect: DialectKind,
    segments: Vec<ErasedSegment>,
    raw: OnceCell<SmolStr>,
    source_fixes: Vec<SourceFix>,
    descendant_type_set: OnceCell<SyntaxSet>,
    raw_segments_with_ancestors: OnceCell<Vec<(ErasedSegment, Vec<PathStep>)>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenData {
    raw: SmolStr,
}

#[track_caller]
pub fn pos_marker(segments: &[ErasedSegment]) -> PositionMarker {
    let markers = segments.iter().filter_map(|seg| seg.get_position_marker());

    PositionMarker::from_child_markers(markers)
}

#[derive(Debug, Clone)]
pub struct PathStep {
    pub segment: ErasedSegment,
    pub idx: usize,
    pub len: usize,
    pub code_idxs: Rc<Vec<usize>>,
}

fn class_types(syntax_kind: SyntaxKind) -> SyntaxSet {
    match syntax_kind {
        SyntaxKind::ColumnReference => SyntaxSet::new(&[SyntaxKind::ObjectReference, syntax_kind]),
        SyntaxKind::WildcardIdentifier => {
            SyntaxSet::new(&[SyntaxKind::WildcardIdentifier, SyntaxKind::ObjectReference])
        }
        SyntaxKind::TableReference => SyntaxSet::new(&[SyntaxKind::ObjectReference, syntax_kind]),
        _ => SyntaxSet::single(syntax_kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint_fix::LintFix;
    use crate::linter::compute_anchor_edit_info;
    use crate::parser::segments::test_functions::{raw_seg, raw_segments};

    fn file_segment(id: u32, segments: Vec<ErasedSegment>) -> ErasedSegment {
        SegmentBuilder::node(id, SyntaxKind::File, DialectKind::Ansi, segments)
            .position_from_segments()
            .finish()
    }

    fn unique_raw_segments(raws: &[&str]) -> Vec<ErasedSegment> {
        let tables = Tables::default();
        let raw = raws.concat();
        let templated_file: TemplatedFile = raw.into();
        let mut offset = 0;
        let mut segments = Vec::with_capacity(raws.len());

        for raw in raws {
            let position = PositionMarker::new(
                offset..offset + raw.len(),
                offset..offset + raw.len(),
                templated_file.clone(),
                None,
                None,
            );
            segments.push(
                SegmentBuilder::token(tables.next_id(), raw, SyntaxKind::RawComparisonOperator)
                    .with_position(position)
                    .finish(),
            );
            offset += raw.len();
        }

        segments
    }

    #[test]
    /// Test comparison of raw segments.
    fn test_parser_base_segments_raw_compare() {
        let template: TemplatedFile = "foobar".into();
        let rs1 = SegmentBuilder::token(0, "foobar", SyntaxKind::Word)
            .with_position(PositionMarker::new(
                0..6,
                0..6,
                template.clone(),
                None,
                None,
            ))
            .finish();
        let rs2 = SegmentBuilder::token(0, "foobar", SyntaxKind::Word)
            .with_position(PositionMarker::new(
                0..6,
                0..6,
                template.clone(),
                None,
                None,
            ))
            .finish();

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

        let mut anchor_edit_info = Default::default();
        compute_anchor_edit_info(&mut anchor_edit_info, fixes);

        // Check the target segment is the only key we have.
        assert_eq!(
            anchor_edit_info.keys().collect::<Vec<_>>(),
            vec![&raw_segs[0].id()]
        );

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
            anchor_info.fixes[anchor_info.first_replace.unwrap()],
            LintFix::replace(
                raw_segs[0].clone(),
                vec![raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None)],
                None,
            )
        );
    }

    #[test]
    fn test_apply_fixes_keeps_anchor_once_for_before_after_pair() {
        let tables = Tables::default();
        let raw_segs = unique_raw_segments(&["foobar", ".barfoo"]);
        let parent = file_segment(tables.next_id(), raw_segs.clone());

        let before = raw_segs[0].edit(tables.next_id(), Some("before".to_string()), None);
        let after = raw_segs[0].edit(tables.next_id(), Some("after".to_string()), None);

        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![
                LintFix::create_after(raw_segs[0].clone(), vec![after], None),
                LintFix::create_before(raw_segs[0].clone(), vec![before]),
            ],
        );

        let (fixed, pre, post) = parent.apply_fixes(&mut fixes);
        assert!(pre.is_empty());
        assert!(post.is_empty());

        let raws = fixed
            .segments()
            .iter()
            .map(|segment| segment.raw().to_string())
            .collect::<Vec<_>>();

        assert_eq!(raws, vec!["before", "foobar", "after", ".barfoo"]);
        assert_eq!(
            raws.iter().filter(|raw| raw.as_str() == "foobar").count(),
            1
        );
    }

    #[test]
    fn test_apply_fixes_leaves_unrelated_fixes_in_map() {
        let tables = Tables::default();
        let raw_segs = unique_raw_segments(&["foobar", ".barfoo"]);
        let parent = file_segment(tables.next_id(), raw_segs.clone());
        let external =
            SegmentBuilder::token(10_000, "external", SyntaxKind::RawComparisonOperator).finish();

        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![
                LintFix::replace(
                    raw_segs[0].clone(),
                    vec![raw_segs[0].edit(tables.next_id(), Some("fixed".to_string()), None)],
                    None,
                ),
                LintFix::replace(
                    external.clone(),
                    vec![
                        SegmentBuilder::token(10_001, "unused", SyntaxKind::RawComparisonOperator)
                            .finish(),
                    ],
                    None,
                ),
            ],
        );

        let (fixed, _, _) = parent.apply_fixes(&mut fixes);
        assert_eq!(
            fixed.segments()[0].raw().as_str(),
            "fixed",
            "the in-subtree fix should still apply",
        );
        assert_eq!(fixes.len(), 1);
        assert!(fixes.contains_key(&external.id()));
    }

    #[test]
    fn test_apply_fixes_preserves_multiple_create_before_anchor_emission() {
        let tables = Tables::default();
        let raw_segs = unique_raw_segments(&["foobar", ".barfoo"]);
        let parent = file_segment(tables.next_id(), raw_segs.clone());

        let before_a = raw_segs[0].edit(tables.next_id(), Some("a".to_string()), None);
        let before_b = raw_segs[0].edit(tables.next_id(), Some("b".to_string()), None);

        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![
                LintFix::create_before(raw_segs[0].clone(), vec![before_a]),
                LintFix::create_before(raw_segs[0].clone(), vec![before_b]),
            ],
        );

        let (fixed, _, _) = parent.apply_fixes(&mut fixes);
        let raws = fixed
            .segments()
            .iter()
            .map(|segment| segment.raw().to_string())
            .collect::<Vec<_>>();

        assert_eq!(raws, vec!["a", "foobar", "b", "foobar", ".barfoo"]);
    }

    #[test]
    fn test_apply_fixes_consumes_descendant_fix_once_across_repeated_keep_anchor_emission() {
        let tables = Tables::default();
        let templated_file: TemplatedFile = "footail".into();
        let child =
            SegmentBuilder::token(tables.next_id(), "foo", SyntaxKind::RawComparisonOperator)
                .with_position(PositionMarker::new(
                    0..3,
                    0..3,
                    templated_file.clone(),
                    None,
                    None,
                ))
                .finish();
        let anchor = SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::SelectStatement,
            DialectKind::Ansi,
            vec![child.clone()],
        )
        .position_from_segments()
        .finish();
        let tail =
            SegmentBuilder::token(tables.next_id(), "tail", SyntaxKind::RawComparisonOperator)
                .with_position(PositionMarker::new(3..7, 3..7, templated_file, None, None))
                .finish();
        let parent = file_segment(tables.next_id(), vec![anchor.clone(), tail]);

        let before_a =
            SegmentBuilder::token(tables.next_id(), "a", SyntaxKind::RawComparisonOperator)
                .finish();
        let before_b =
            SegmentBuilder::token(tables.next_id(), "b", SyntaxKind::RawComparisonOperator)
                .finish();
        let replaced_child = child.edit(tables.next_id(), Some("bar".to_string()), None);

        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![
                LintFix::create_before(anchor.clone(), vec![before_a]),
                LintFix::create_before(anchor.clone(), vec![before_b]),
                LintFix::replace(child, vec![replaced_child], None),
            ],
        );

        let fixed = parent.apply_fixes_v2(fixes);
        let raws = fixed
            .segments()
            .iter()
            .map(|segment| segment.raw().to_string())
            .collect::<Vec<_>>();

        assert_eq!(raws, vec!["a", "bar", "b", "foo", "tail"]);
    }

    #[test]
    fn test_apply_fixes_replacement_adopts_anchor_position_on_first_raw_match() {
        let tables = Tables::default();
        let raw_segs = unique_raw_segments(&["foobar", ".barfoo"]);
        let parent = file_segment(tables.next_id(), raw_segs.clone());
        let anchor_pos = raw_segs[0].get_position_marker().unwrap().clone();

        let replacement_same_raw =
            raw_segs[0].edit(tables.next_id(), Some("foobar".to_string()), None);
        let replacement_extra = raw_segs[0].edit(tables.next_id(), Some("zz".to_string()), None);

        let mut fixes = HashMap::new();
        compute_anchor_edit_info(
            &mut fixes,
            vec![LintFix::replace(
                raw_segs[0].clone(),
                vec![replacement_same_raw, replacement_extra],
                None,
            )],
        );

        let fixed = parent.apply_fixes_v2(fixes);
        let children = fixed.segments();

        assert_eq!(children[0].raw().as_str(), "foobar");
        assert_eq!(
            children[0].get_position_marker().unwrap().source_slice,
            anchor_pos.source_slice
        );
        assert_eq!(
            children[0].get_position_marker().unwrap().templated_slice,
            anchor_pos.templated_slice
        );
        assert_eq!(children[0].get_start_loc(), anchor_pos.working_loc());
        assert_eq!(children[1].raw().as_str(), "zz");
        assert_eq!(children[1].get_start_loc(), (1, 7));
        assert_eq!(children[2].get_start_loc(), (1, 9));
    }

    #[test]
    fn test_position_segments_positions_runs_of_unpositioned_siblings_linearly() {
        let tables = Tables::default();
        let raw_segs = unique_raw_segments(&["foobar", ".barfoo"]);
        let parent = file_segment(tables.next_id(), raw_segs.clone());

        let mut inserted_a = raw_segs[0].edit(tables.next_id(), Some("x".to_string()), None);
        inserted_a.make_mut().set_position_marker(None);

        let mut inserted_b = raw_segs[0].edit(tables.next_id(), Some("yy".to_string()), None);
        inserted_b.make_mut().set_position_marker(None);

        let positioned = position_segments(
            &[
                raw_segs[0].clone(),
                inserted_a,
                inserted_b,
                raw_segs[1].clone(),
            ],
            parent.get_position_marker().unwrap(),
        );

        let starts = positioned
            .iter()
            .map(ErasedSegment::get_start_loc)
            .collect::<Vec<_>>();

        assert_eq!(starts, vec![(1, 1), (1, 7), (1, 8), (1, 10)]);
    }

    #[test]
    fn test_position_segments_uses_first_raw_descendant_for_right_anchor() {
        let tables = Tables::default();
        let templated_file: TemplatedFile = "___bar".into();
        let raw = SegmentBuilder::token(tables.next_id(), "bar", SyntaxKind::RawComparisonOperator)
            .with_position(PositionMarker::new(
                3..6,
                3..6,
                templated_file.clone(),
                None,
                None,
            ))
            .finish();
        let inner = SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::Bracketed,
            DialectKind::Ansi,
            vec![raw],
        )
        .with_position(PositionMarker::new(
            0..6,
            0..6,
            templated_file.clone(),
            None,
            None,
        ))
        .finish();
        let node = SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::SelectStatement,
            DialectKind::Ansi,
            vec![inner],
        )
        .position_from_segments()
        .finish();
        let parent = file_segment(tables.next_id(), vec![node.clone()]);

        let mut inserted =
            SegmentBuilder::token(tables.next_id(), "x", SyntaxKind::RawComparisonOperator)
                .finish();
        inserted.make_mut().set_position_marker(None);

        let positioned =
            position_segments(&[inserted, node], parent.get_position_marker().unwrap());

        assert_eq!(
            positioned[0].get_position_marker().unwrap().source_slice,
            0..3
        );
        assert_eq!(
            positioned[0].get_position_marker().unwrap().templated_slice,
            0..3
        );
    }

    #[test]
    fn test_text_extent_matches_infer_next_position() {
        let samples = [
            "", "a", "\n", "\r", "\r\n", "é", "🙂", "\t", "a\n", "a\r\n", "\n🙂", "é\n🙂",
        ];

        for left in samples {
            for right in samples {
                let raw = format!("{left}{right}");
                let extent = TextExtent::from_raw(&raw);
                for (line_no, line_pos) in [(1, 1), (2, 4), (5, 9)] {
                    assert_eq!(
                        extent.advance(line_no, line_pos),
                        PositionMarker::infer_next_position(&raw, line_no, line_pos),
                        "raw={raw:?}, start=({line_no}, {line_pos})",
                    );
                }
            }
        }
    }

    #[test]
    fn test_position_only_freeze_matches_old_cache_policy() {
        let tables = Tables::default();
        let raw = unique_raw_segments(&["foo"]);
        let node = SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::SelectStatement,
            DialectKind::Ansi,
            raw,
        )
        .position_from_segments()
        .finish();

        let _ = node.raw();
        let _ = node.descendant_type_set();
        let _ = node.raw_segments_with_ancestors();
        let _ = node.code_indices();
        let _ = node.hash_value();

        let moved = position_only_freeze_base(
            &node,
            node.get_position_marker()
                .unwrap()
                .clone()
                .with_working_position(3, 7),
        );

        let NodeOrTokenKind::Node(node_data) = &moved.value.kind else {
            panic!("expected node");
        };

        assert!(moved.value.code_idx.get().is_none());
        assert!(moved.value.hash.get().is_none());
        assert!(node_data.raw.get().is_some());
        assert!(node_data.descendant_type_set.get().is_some());
        assert!(node_data.raw_segments_with_ancestors.get().is_some());
    }
}
