use std::ops::Deref;
use std::rc::Rc;

use ahash::{AHashMap, AHashSet};
use fancy_regex::Regex;
use itertools::{Itertools as _, enumerate, multiunzip};
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use thiserror::Error;

use super::IndentationConfig;
use super::match_algorithms::{
    first_non_whitespace, first_trimmed_raw, skip_start_index_forward_to_code,
    skip_stop_index_backward_to_code,
};
use super::match_result::{MatchResult, Matched, Span};
use super::matchable::{Matchable, MatchableTrait, MatchableTraitImpl};
use super::segments::{ErasedSegment, SegmentBuilder, Tables};
use crate::dialects::Dialect;
use crate::dialects::init::DialectKind;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;
use crate::helpers::IndexSet;

pub type SymbolId = u32;

type LocKey = u32;
type LocKeyData = (usize, usize, SyntaxKind, u32);
type BracketMatch = Result<(MatchResult, Option<NodeId>, Vec<MatchResult>), SQLParseError>;
type SimpleSet = (AHashSet<String>, SyntaxSet);

#[derive(Default)]
struct NextMatchPrepared {
    raw_simple_map: AHashMap<String, Vec<usize>>,
    type_simple_map: AHashMap<SyntaxKind, Vec<usize>>,
    type_simple_keys: SyntaxSet,
}

struct NextExBracketPrepared {
    start_brackets: Vec<NodeId>,
    end_brackets: Vec<NodeId>,
    bracket_persists: Vec<bool>,
    all_matchers: Vec<NodeId>,
    next_match_prepared: NextMatchPrepared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Sequence,
    OneOf,
    AnyNumberOf,
    Ref,
    NodeMatcher,
    String,
    MultiString,
    Regex,
    Typed,
    Code,
    NonCode,
    Nothing,
    Anything,
    Delimited,
    Bracketed,
    Meta,
    Conditional,
    BracketedSegmentMatcher,
    LookaheadExclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub kind: Kind,
    pub a: u32,
    pub b: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NodeSlice {
    start: u32,
    len: u32,
}

impl NodeSlice {
    #[inline]
    fn is_empty(self) -> bool {
        self.len == 0
    }

    #[inline]
    fn as_slice(self, kids: &[NodeId]) -> &[NodeId] {
        &kids[self.start as usize..(self.start + self.len) as usize]
    }
}

#[derive(Debug, Clone)]
struct SequencePayload {
    parse_mode: ParseMode,
    allow_gaps: bool,
    optional: bool,
    terminators: NodeSlice,
}

#[derive(Debug, Clone)]
struct AnyNumberOfPayload {
    exclude: Option<NodeId>,
    terminators: NodeSlice,
    reset_terminators: bool,
    max_times: Option<usize>,
    min_times: usize,
    max_times_per_element: Option<usize>,
    allow_gaps: bool,
    optional: bool,
    parse_mode: ParseMode,
}

#[derive(Debug, Clone)]
struct RefPayload {
    symbol: SymbolId,
    exclude: Option<NodeId>,
    terminators: NodeSlice,
    reset_terminators: bool,
    optional: bool,
    resolved: Option<NodeId>,
}

#[derive(Debug, Clone, Copy)]
struct NodeMatcherPayload {
    node_kind: SyntaxKind,
    child: NodeId,
}

#[derive(Debug, Clone)]
struct StringPayload {
    template: u32,
    kind: SyntaxKind,
    optional: bool,
}

#[derive(Debug, Clone)]
struct MultiStringPayload {
    templates: NodeSlice,
    kind: SyntaxKind,
}

#[derive(Debug, Clone, Copy)]
struct RegexPayload {
    regex_id: u32,
    kind: SyntaxKind,
}

#[derive(Debug, Clone, Copy)]
struct TypedPayload {
    template: SyntaxKind,
    kind: SyntaxKind,
    optional: bool,
}

#[derive(Debug, Clone)]
struct AnythingPayload {
    terminators: NodeSlice,
}

#[derive(Debug, Clone)]
struct DelimitedPayload {
    allow_trailing: bool,
    delimiter: NodeId,
    min_delimiters: usize,
    optional_delimiter: bool,
    optional: bool,
    allow_gaps: bool,
    terminators: NodeSlice,
}

#[derive(Debug, Clone)]
struct BracketedPayload {
    bracket_type: SymbolId,
    bracket_pairs_set: SymbolId,
    allow_gaps: bool,
    parse_mode: ParseMode,
    inner: NodeId,
}

#[derive(Debug, Clone, Copy)]
struct MetaPayload {
    kind: SyntaxKind,
}

#[derive(Debug, Clone, Copy)]
struct ConditionalPayload {
    meta: SyntaxKind,
    requirements: IndentationConfig,
}

#[derive(Debug, Clone, Copy)]
struct LookaheadExcludePayload {
    first_token: u32,
    lookahead_token: u32,
}

#[derive(Debug, Clone)]
struct RegexEntry {
    regex: Regex,
    anti_regex: Option<Regex>,
}

#[derive(Debug, Clone)]
enum Payload {
    None,
    Sequence(SequencePayload),
    AnyNumberOf(AnyNumberOfPayload),
    Ref(RefPayload),
    NodeMatcher(NodeMatcherPayload),
    String(StringPayload),
    MultiString(MultiStringPayload),
    Regex(RegexPayload),
    Typed(TypedPayload),
    Anything(AnythingPayload),
    Delimited(DelimitedPayload),
    Bracketed(BracketedPayload),
    Meta(MetaPayload),
    Conditional(ConditionalPayload),
    LookaheadExclude(LookaheadExcludePayload),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Strict,
    Greedy,
    GreedyOnceStarted,
}

impl From<super::types::ParseMode> for ParseMode {
    fn from(value: super::types::ParseMode) -> Self {
        match value {
            super::types::ParseMode::Strict => Self::Strict,
            super::types::ParseMode::Greedy => Self::Greedy,
            super::types::ParseMode::GreedyOnceStarted => Self::GreedyOnceStarted,
        }
    }
}

#[derive(Debug, Error, Clone)]
pub enum CompileError {
    #[error("dialect still contains SegmentGenerator for '{0}'")]
    SegmentGenerator(String),
    #[error("missing grammar reference '{0}'")]
    MissingReference(String),
    #[error("unsupported grammar shape: {0}")]
    Unsupported(String),
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    loc: LocKey,
    key: u32,
}

impl CacheKey {
    #[inline]
    fn new(loc: LocKey, key: u32) -> Self {
        Self { loc, key }
    }
}

#[derive(Debug)]
struct CompiledParseContext<'a> {
    grammar: &'a CompiledGrammar,
    dialect: &'a Dialect,
    terminators: Vec<NodeId>,
    loc_keys: IndexSet<LocKeyData>,
    parse_cache: FxHashMap<CacheKey, MatchResult>,
    simple_cache: FxHashMap<NodeId, Option<Rc<SimpleSet>>>,
    indentation_config: IndentationConfig,
}

impl<'a> CompiledParseContext<'a> {
    fn new(
        grammar: &'a CompiledGrammar,
        dialect: &'a Dialect,
        indentation_config: IndentationConfig,
    ) -> Self {
        Self {
            grammar,
            dialect,
            terminators: Vec::new(),
            loc_keys: IndexSet::default(),
            parse_cache: FxHashMap::default(),
            simple_cache: FxHashMap::default(),
            indentation_config,
        }
    }

    #[inline]
    fn deeper_match<T>(
        &mut self,
        clear_terminators: bool,
        push_terminators: &[NodeId],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let (appended, terms) = self.set_terminators(clear_terminators, push_terminators);
        let ret = f(self);
        self.reset_terminators(appended, terms, clear_terminators);
        ret
    }

    fn set_terminators(
        &mut self,
        clear_terminators: bool,
        push_terminators: &[NodeId],
    ) -> (usize, Vec<NodeId>) {
        let mut appended = 0;
        let terminators = if clear_terminators {
            self.terminators.clone()
        } else {
            Vec::new()
        };

        if clear_terminators && !self.terminators.is_empty() {
            self.terminators = if !push_terminators.is_empty() {
                push_terminators.to_vec()
            } else {
                Vec::new()
            };
        } else if !push_terminators.is_empty() {
            for &terminator in push_terminators {
                let already_present = self.terminators.iter().any(|&existing| {
                    self.grammar.node_eq_group(existing) == self.grammar.node_eq_group(terminator)
                });

                if !already_present {
                    self.terminators.push(terminator);
                    appended += 1;
                }
            }
        }

        (appended, terminators)
    }

    fn reset_terminators(
        &mut self,
        appended: usize,
        terminators: Vec<NodeId>,
        clear_terminators: bool,
    ) {
        if clear_terminators {
            self.terminators = terminators;
        } else {
            let new_len = self.terminators.len().saturating_sub(appended);
            self.terminators.truncate(new_len);
        }
    }

    #[inline]
    fn loc_key(&mut self, data: LocKeyData) -> LocKey {
        let (key, _) = self.loc_keys.insert_full(data);
        key as u32
    }

    #[inline]
    fn check_parse_cache(&self, loc_key: LocKey, matcher_key: u32) -> Option<&MatchResult> {
        self.parse_cache.get(&CacheKey::new(loc_key, matcher_key))
    }

    #[inline]
    fn put_parse_cache(
        &mut self,
        loc_key: LocKey,
        matcher_key: u32,
        match_result: MatchResult,
    ) -> &MatchResult {
        self.parse_cache
            .entry(CacheKey::new(loc_key, matcher_key))
            .or_insert(match_result)
    }
}

#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    nodes: Vec<Node>,
    payloads: Vec<Payload>,
    node_eq_groups: Vec<u32>,
    next_node_eq_group: u32,
    kids: Vec<NodeId>,
    symbols: Vec<SmolStr>,
    symbol_index: AHashMap<SmolStr, SymbolId>,
    definitions: Vec<Option<NodeId>>,
    strings: Vec<SmolStr>,
    string_index: AHashMap<SmolStr, u32>,
    regexes: Vec<RegexEntry>,
    regex_index: AHashMap<(SmolStr, Option<SmolStr>), u32>,
    builtin_non_code: NodeId,
    compiled: bool,
}

impl Default for CompiledGrammar {
    fn default() -> Self {
        Self::new()
    }
}

impl CompiledGrammar {
    pub fn new() -> Self {
        let mut this = Self {
            nodes: Vec::new(),
            payloads: Vec::new(),
            node_eq_groups: Vec::new(),
            next_node_eq_group: 0,
            kids: Vec::new(),
            symbols: Vec::new(),
            symbol_index: AHashMap::new(),
            definitions: Vec::new(),
            strings: Vec::new(),
            string_index: AHashMap::new(),
            regexes: Vec::new(),
            regex_index: AHashMap::new(),
            builtin_non_code: NodeId(0),
            compiled: false,
        };

        let builtin_non_code = this.push_node(Kind::NonCode, 0, 0, Payload::None);
        this.builtin_non_code = builtin_non_code;
        this
    }

    pub fn from_dialect(dialect: &Dialect) -> Result<Self, CompileError> {
        let mut compiler = LegacyCompiler {
            dialect,
            grammar: Self::new(),
            seen: AHashMap::new(),
            eq_representatives: Vec::new(),
            next_eq_group: 0,
        };

        if let Some(name) = dialect.segment_generator_names().into_iter().next() {
            return Err(CompileError::SegmentGenerator(name.into_owned()));
        }

        for (name, matcher) in dialect.matchable_entries() {
            let node_id = compiler.compile_matchable(&matcher)?;
            compiler.grammar.define(name.as_ref(), node_id);
        }

        compiler.grammar.compile()
    }

    pub fn compile(mut self) -> Result<Self, CompileError> {
        self.resolve_refs()?;
        self.normalize();
        self.compiled = true;
        Ok(self)
    }

    pub fn define(&mut self, name: impl Into<SmolStr>, node: NodeId) {
        let name: SmolStr = name.into();
        let symbol = self.intern_symbol(name.as_str());
        self.ensure_definitions_len(symbol);
        self.definitions[symbol as usize] = Some(node);
    }

    pub fn ref_(&mut self, name: impl AsRef<str>) -> NodeId {
        let symbol = self.intern_symbol(name.as_ref());
        let payload = RefPayload {
            symbol,
            exclude: None,
            terminators: NodeSlice::default(),
            reset_terminators: false,
            optional: false,
            resolved: None,
        };

        self.push_node(Kind::Ref, symbol, 0, Payload::Ref(payload))
    }

    pub fn keyword(&mut self, keyword: impl AsRef<str>) -> NodeId {
        let keyword = keyword.as_ref().to_ascii_uppercase();
        self.string(&keyword, SyntaxKind::Keyword)
    }

    pub fn string(&mut self, template: impl AsRef<str>, kind: SyntaxKind) -> NodeId {
        let template_id = self.intern_string(template.as_ref().to_ascii_uppercase());
        let payload = StringPayload {
            template: template_id,
            kind,
            optional: false,
        };

        self.push_node(
            Kind::String,
            template_id,
            kind as u32,
            Payload::String(payload),
        )
    }

    pub fn regex(
        &mut self,
        pattern: impl AsRef<str>,
        anti_pattern: Option<impl AsRef<str>>,
        kind: SyntaxKind,
    ) -> NodeId {
        let anti_pattern = anti_pattern.map(|it| it.as_ref().to_owned());
        let regex_id = self.intern_regex(pattern.as_ref(), anti_pattern.as_deref());
        let payload = RegexPayload { regex_id, kind };

        self.push_node(Kind::Regex, regex_id, kind as u32, Payload::Regex(payload))
    }

    pub fn typed(&mut self, template: SyntaxKind, kind: SyntaxKind) -> NodeId {
        let payload = TypedPayload {
            template,
            kind,
            optional: false,
        };

        self.push_node(
            Kind::Typed,
            template as u32,
            kind as u32,
            Payload::Typed(payload),
        )
    }

    pub fn code(&mut self) -> NodeId {
        self.push_node(Kind::Code, 0, 0, Payload::None)
    }

    pub fn non_code(&self) -> NodeId {
        self.builtin_non_code
    }

    pub fn nothing(&mut self) -> NodeId {
        self.push_node(Kind::Nothing, 0, 0, Payload::None)
    }

    pub fn sequence<I>(&mut self, children: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let slice = self.push_children(children);
        let payload = SequencePayload {
            parse_mode: ParseMode::Strict,
            allow_gaps: true,
            optional: false,
            terminators: NodeSlice::default(),
        };
        self.push_node(
            Kind::Sequence,
            slice.start,
            slice.len,
            Payload::Sequence(payload),
        )
    }

    pub fn one_of<I>(&mut self, children: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let slice = self.push_children(children);
        let payload = AnyNumberOfPayload {
            exclude: None,
            terminators: NodeSlice::default(),
            reset_terminators: false,
            max_times: Some(1),
            min_times: 1,
            max_times_per_element: None,
            allow_gaps: true,
            optional: false,
            parse_mode: ParseMode::Strict,
        };

        self.push_node(
            Kind::OneOf,
            slice.start,
            slice.len,
            Payload::AnyNumberOf(payload),
        )
    }

    pub fn node_matcher(&mut self, kind: SyntaxKind, child: NodeId) -> NodeId {
        let payload = NodeMatcherPayload {
            node_kind: kind,
            child,
        };

        self.push_node(
            Kind::NodeMatcher,
            kind as u32,
            child.0,
            Payload::NodeMatcher(payload),
        )
    }

    pub fn root(&self, name: &str) -> Option<NodeId> {
        let symbol = self.symbol_index.get(name)?;
        self.definitions[*symbol as usize]
    }

    pub fn root_parse_file(
        &self,
        tables: &Tables,
        dialect: DialectKind,
        dialect_ref: &Dialect,
        segments: &[ErasedSegment],
        indentation_config: IndentationConfig,
    ) -> Result<ErasedSegment, SQLParseError> {
        let start_idx = segments
            .iter()
            .position(|segment| segment.is_code())
            .unwrap_or(0) as u32;

        let end_idx = segments
            .iter()
            .rposition(|segment| segment.is_code())
            .map_or(start_idx, |idx| idx as u32 + 1);

        if start_idx == end_idx {
            return Ok(SegmentBuilder::node(
                tables.next_id(),
                SyntaxKind::File,
                dialect,
                segments.to_vec(),
            )
            .position_from_segments()
            .finish());
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let file_node = self
            .root("FileSegment")
            .ok_or_else(|| SQLParseError::new("missing FileSegment root"))?;

        let entry = match self.node(file_node).kind {
            Kind::NodeMatcher => self.node_matcher_payload(file_node).child,
            _ => file_node,
        };

        let mut ctx = CompiledParseContext::new(self, dialect_ref, indentation_config);
        let match_result =
            self.match_node(entry, &segments[..end_idx as usize], start_idx, &mut ctx)?;

        let match_span = match_result.span;
        let has_match = match_result.has_match();
        let mut matched = match_result.apply(tables, dialect, segments);
        let unmatched = &segments[match_span.end as usize..end_idx as usize];

        let content: &[ErasedSegment] = if !has_match {
            &[SegmentBuilder::node(
                tables.next_id(),
                SyntaxKind::Unparsable,
                dialect,
                segments[start_idx as usize..end_idx as usize].to_vec(),
            )
            .position_from_segments()
            .finish()]
        } else if !unmatched.is_empty() {
            let idx = unmatched
                .iter()
                .position(|it| it.is_code())
                .unwrap_or(unmatched.len());
            let (head, tail) = unmatched.split_at(idx);

            matched.extend_from_slice(head);
            matched.push(
                SegmentBuilder::node(
                    tables.next_id(),
                    SyntaxKind::Unparsable,
                    dialect,
                    tail.to_vec(),
                )
                .position_from_segments()
                .finish(),
            );
            &matched
        } else {
            matched.extend_from_slice(unmatched);
            &matched
        };

        Ok(SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::File,
            dialect,
            [
                &segments[..start_idx as usize],
                content,
                &segments[end_idx as usize..],
            ]
            .concat(),
        )
        .position_from_segments()
        .finish())
    }

    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.as_usize()]
    }

    fn payload(&self, id: NodeId) -> &Payload {
        &self.payloads[id.as_usize()]
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        let node = self.node(id);
        NodeSlice {
            start: node.a,
            len: node.b,
        }
        .as_slice(&self.kids)
    }

    fn node_matcher_payload(&self, id: NodeId) -> NodeMatcherPayload {
        match self.payload(id) {
            Payload::NodeMatcher(payload) => *payload,
            _ => unreachable!("node {:?} is not NodeMatcher", self.node(id).kind),
        }
    }

    fn sequence_payload(&self, id: NodeId) -> &SequencePayload {
        match self.payload(id) {
            Payload::Sequence(payload) => payload,
            _ => unreachable!("node {:?} is not Sequence", self.node(id).kind),
        }
    }

    fn any_number_of_payload(&self, id: NodeId) -> &AnyNumberOfPayload {
        match self.payload(id) {
            Payload::AnyNumberOf(payload) => payload,
            _ => unreachable!("node {:?} is not AnyNumberOf", self.node(id).kind),
        }
    }

    fn ref_payload(&self, id: NodeId) -> &RefPayload {
        match self.payload(id) {
            Payload::Ref(payload) => payload,
            _ => unreachable!("node {:?} is not Ref", self.node(id).kind),
        }
    }

    fn string_payload(&self, id: NodeId) -> &StringPayload {
        match self.payload(id) {
            Payload::String(payload) => payload,
            _ => unreachable!("node {:?} is not String", self.node(id).kind),
        }
    }

    fn multi_string_payload(&self, id: NodeId) -> &MultiStringPayload {
        match self.payload(id) {
            Payload::MultiString(payload) => payload,
            _ => unreachable!("node {:?} is not MultiString", self.node(id).kind),
        }
    }

    fn regex_payload(&self, id: NodeId) -> RegexPayload {
        match self.payload(id) {
            Payload::Regex(payload) => *payload,
            _ => unreachable!("node {:?} is not Regex", self.node(id).kind),
        }
    }

    fn typed_payload(&self, id: NodeId) -> TypedPayload {
        match self.payload(id) {
            Payload::Typed(payload) => *payload,
            _ => unreachable!("node {:?} is not Typed", self.node(id).kind),
        }
    }

    fn anything_payload(&self, id: NodeId) -> &AnythingPayload {
        match self.payload(id) {
            Payload::Anything(payload) => payload,
            _ => unreachable!("node {:?} is not Anything", self.node(id).kind),
        }
    }

    fn delimited_payload(&self, id: NodeId) -> &DelimitedPayload {
        match self.payload(id) {
            Payload::Delimited(payload) => payload,
            _ => unreachable!("node {:?} is not Delimited", self.node(id).kind),
        }
    }

    fn bracketed_payload(&self, id: NodeId) -> &BracketedPayload {
        match self.payload(id) {
            Payload::Bracketed(payload) => payload,
            _ => unreachable!("node {:?} is not Bracketed", self.node(id).kind),
        }
    }

    fn meta_payload(&self, id: NodeId) -> MetaPayload {
        match self.payload(id) {
            Payload::Meta(payload) => *payload,
            _ => unreachable!("node {:?} is not Meta", self.node(id).kind),
        }
    }

    fn conditional_payload(&self, id: NodeId) -> ConditionalPayload {
        match self.payload(id) {
            Payload::Conditional(payload) => *payload,
            _ => unreachable!("node {:?} is not Conditional", self.node(id).kind),
        }
    }

    fn lookahead_payload(&self, id: NodeId) -> LookaheadExcludePayload {
        match self.payload(id) {
            Payload::LookaheadExclude(payload) => *payload,
            _ => unreachable!("node {:?} is not LookaheadExclude", self.node(id).kind),
        }
    }

    fn node_cache_key(&self, id: NodeId) -> u32 {
        id.0 | 0x8000_0000
    }

    #[inline]
    fn node_eq_group(&self, id: NodeId) -> u32 {
        self.node_eq_groups[id.as_usize()]
    }

    #[inline]
    fn set_node_eq_group(&mut self, id: NodeId, group: u32) {
        self.node_eq_groups[id.as_usize()] = group;
    }

    fn symbol_name(&self, symbol: SymbolId) -> &str {
        &self.symbols[symbol as usize]
    }

    fn string_value(&self, id: u32) -> &str {
        &self.strings[id as usize]
    }

    fn get_definition_by_name(&self, name: &str) -> Option<NodeId> {
        let symbol = self.symbol_index.get(name)?;
        self.definitions[*symbol as usize]
    }

    fn intern_symbol(&mut self, name: impl AsRef<str>) -> SymbolId {
        let name = SmolStr::new(name.as_ref());
        if let Some(&id) = self.symbol_index.get(&name) {
            return id;
        }

        let id = self.symbols.len() as u32;
        self.symbols.push(name.clone());
        self.symbol_index.insert(name, id);
        self.ensure_definitions_len(id);
        id
    }

    fn intern_string(&mut self, value: impl AsRef<str>) -> u32 {
        let value = SmolStr::new(value.as_ref());
        if let Some(&id) = self.string_index.get(&value) {
            return id;
        }

        let id = self.strings.len() as u32;
        self.strings.push(value.clone());
        self.string_index.insert(value, id);
        id
    }

    fn intern_regex(&mut self, pattern: &str, anti_pattern: Option<&str>) -> u32 {
        let key = (SmolStr::new(pattern), anti_pattern.map(SmolStr::new));
        if let Some(&id) = self.regex_index.get(&key) {
            return id;
        }

        let regex = Regex::new(pattern).unwrap();
        let anti_regex = anti_pattern.map(|it| Regex::new(it).unwrap());

        let id = self.regexes.len() as u32;
        self.regexes.push(RegexEntry { regex, anti_regex });
        self.regex_index.insert(key, id);
        id
    }

    fn ensure_definitions_len(&mut self, symbol: SymbolId) {
        let required = symbol as usize + 1;
        if self.definitions.len() < required {
            self.definitions.resize(required, None);
        }
    }

    fn push_children<I>(&mut self, children: I) -> NodeSlice
    where
        I: IntoIterator<Item = NodeId>,
    {
        let start = self.kids.len() as u32;
        self.kids.extend(children);
        let len = self.kids.len() as u32 - start;

        NodeSlice { start, len }
    }

    fn push_node(&mut self, kind: Kind, a: u32, b: u32, payload: Payload) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node { kind, a, b });
        self.payloads.push(payload);
        self.node_eq_groups.push(self.next_node_eq_group);
        self.next_node_eq_group = self.next_node_eq_group.saturating_add(1);
        id
    }

    fn make_sequence(
        &mut self,
        children: Vec<NodeId>,
        parse_mode: ParseMode,
        allow_gaps: bool,
        optional: bool,
        terminators: Vec<NodeId>,
    ) -> NodeId {
        let child_slice = self.push_children(children);
        let terminators = self.push_children(terminators);

        self.push_node(
            Kind::Sequence,
            child_slice.start,
            child_slice.len,
            Payload::Sequence(SequencePayload {
                parse_mode,
                allow_gaps,
                optional,
                terminators,
            }),
        )
    }

    fn make_any_number_of(
        &mut self,
        kind: Kind,
        children: Vec<NodeId>,
        payload: AnyNumberOfPayload,
    ) -> NodeId {
        let child_slice = self.push_children(children);
        self.push_node(
            kind,
            child_slice.start,
            child_slice.len,
            Payload::AnyNumberOf(payload),
        )
    }

    fn kids_slice(&self, slice: NodeSlice) -> &[NodeId] {
        slice.as_slice(&self.kids)
    }

    fn is_implicit_keyword_symbol(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    }

    fn resolve_refs(&mut self) -> Result<(), CompileError> {
        let mut unresolved_symbols = AHashSet::new();
        for payload in &self.payloads {
            let Payload::Ref(ref_payload) = payload else {
                continue;
            };

            let unresolved = self
                .definitions
                .get(ref_payload.symbol as usize)
                .copied()
                .flatten()
                .is_none();

            if unresolved {
                unresolved_symbols.insert(ref_payload.symbol);
            }
        }

        for symbol in unresolved_symbols {
            if self
                .definitions
                .get(symbol as usize)
                .copied()
                .flatten()
                .is_some()
            {
                continue;
            }

            let symbol_name = self.symbol_name(symbol).to_owned();
            if Self::is_implicit_keyword_symbol(&symbol_name) {
                let keyword_node = self.keyword(&symbol_name);
                self.ensure_definitions_len(symbol);
                self.definitions[symbol as usize] = Some(keyword_node);
            }
        }

        for payload in &mut self.payloads {
            let Payload::Ref(ref_payload) = payload else {
                continue;
            };

            let resolved = self
                .definitions
                .get(ref_payload.symbol as usize)
                .copied()
                .flatten();

            ref_payload.resolved = resolved;
        }

        Ok(())
    }

    fn normalize(&mut self) {
        for node_idx in 0..self.nodes.len() {
            let node_id = NodeId(node_idx as u32);
            match self.node(node_id).kind {
                Kind::Sequence => self.normalize_sequence(node_id),
                Kind::OneOf => self.normalize_one_of(node_id),
                _ => {}
            }
        }
    }

    fn normalize_sequence(&mut self, node_id: NodeId) {
        let payload = self.sequence_payload(node_id).clone();
        let children = self.node_children(node_id);
        let mut flattened: Option<Vec<NodeId>> = None;

        for (idx, child) in children.iter().copied().enumerate() {
            if self.node(child).kind == Kind::Nothing {
                flattened.get_or_insert_with(|| {
                    let mut out = Vec::with_capacity(children.len());
                    out.extend_from_slice(&children[..idx]);
                    out
                });
                continue;
            }

            let can_flatten = self.node(child).kind == Kind::Sequence
                && self
                    .payload(child)
                    .as_sequence()
                    .is_some_and(|child_payload| {
                        child_payload.parse_mode == payload.parse_mode
                            && child_payload.allow_gaps == payload.allow_gaps
                            && !child_payload.optional
                            && child_payload.terminators.is_empty()
                    });

            if can_flatten {
                let flattened = flattened.get_or_insert_with(|| {
                    let mut out = Vec::with_capacity(children.len());
                    out.extend_from_slice(&children[..idx]);
                    out
                });
                flattened.extend_from_slice(self.node_children(child));
            } else if let Some(flattened) = flattened.as_mut() {
                flattened.push(child);
            }
        }

        let Some(flattened) = flattened else {
            return;
        };

        let child_slice = self.push_children(flattened);
        self.nodes[node_id.as_usize()].a = child_slice.start;
        self.nodes[node_id.as_usize()].b = child_slice.len;
    }

    fn normalize_one_of(&mut self, node_id: NodeId) {
        let payload = self.any_number_of_payload(node_id).clone();
        if payload.exclude.is_some()
            || payload.reset_terminators
            || payload.max_times != Some(1)
            || payload.min_times != 1
            || payload.max_times_per_element.is_some()
            || !payload.allow_gaps
            || payload.optional
            || payload.parse_mode != ParseMode::Strict
            || !payload.terminators.is_empty()
        {
            return;
        }

        let children = self.node_children(node_id);
        let mut flattened: Option<Vec<NodeId>> = None;

        for (idx, child) in children.iter().copied().enumerate() {
            let is_plain_one_of = self.node(child).kind == Kind::OneOf
                && self
                    .payload(child)
                    .as_any_number_of()
                    .is_some_and(|child_payload| {
                        child_payload.exclude.is_none()
                            && !child_payload.reset_terminators
                            && child_payload.max_times == Some(1)
                            && child_payload.min_times == 1
                            && child_payload.max_times_per_element.is_none()
                            && child_payload.allow_gaps
                            && !child_payload.optional
                            && child_payload.parse_mode == ParseMode::Strict
                            && child_payload.terminators.is_empty()
                    });

            if is_plain_one_of {
                let flattened = flattened.get_or_insert_with(|| {
                    let mut out = Vec::with_capacity(children.len());
                    out.extend_from_slice(&children[..idx]);
                    out
                });
                flattened.extend_from_slice(self.node_children(child));
            } else if let Some(flattened) = flattened.as_mut() {
                flattened.push(child);
            }
        }

        let Some(flattened) = flattened else {
            return;
        };

        let child_slice = self.push_children(flattened);
        self.nodes[node_id.as_usize()].a = child_slice.start;
        self.nodes[node_id.as_usize()].b = child_slice.len;
    }

    fn simple(
        &self,
        node_id: NodeId,
        parse_context: &mut CompiledParseContext,
        crumbs: Option<Vec<SymbolId>>,
    ) -> Option<Rc<SimpleSet>> {
        let cacheable = crumbs.is_none();
        if cacheable && let Some(cached) = parse_context.simple_cache.get(&node_id) {
            return cached.clone();
        }

        let result = match self.node(node_id).kind {
            Kind::Sequence => {
                let mut simple_raws = AHashSet::new();
                let mut simple_types = SyntaxSet::EMPTY;

                for child in self.node_children(node_id) {
                    let simple = self.simple(*child, parse_context, crumbs.clone())?;
                    let (raws, types) = simple.as_ref();
                    simple_raws.extend(raws.iter().cloned());
                    simple_types = simple_types.union(types);

                    if !self.is_optional(*child) {
                        break;
                    }
                }

                Some(Rc::new((simple_raws, simple_types)))
            }
            Kind::OneOf | Kind::AnyNumberOf | Kind::Delimited => {
                let mut simple_raws = AHashSet::new();
                let mut simple_types = SyntaxSet::EMPTY;

                for child in self.node_children(node_id) {
                    let simple = self.simple(*child, parse_context, crumbs.clone())?;
                    let (raws, types) = simple.as_ref();
                    simple_raws.extend(raws.iter().cloned());
                    simple_types = simple_types.union(types);
                }

                Some(Rc::new((simple_raws, simple_types)))
            }
            Kind::Ref => {
                let payload = self.ref_payload(node_id);
                if let Some(ref c) = crumbs
                    && c.contains(&payload.symbol)
                {
                    let loop_string = c
                        .iter()
                        .map(|id| self.symbol_name(*id))
                        .collect_vec()
                        .join(" -> ");
                    panic!("Self referential grammar detected: {loop_string}");
                }

                let mut new_crumbs = crumbs.unwrap_or_default();
                new_crumbs.push(payload.symbol);

                self.simple(payload.resolved?, parse_context, Some(new_crumbs))
            }
            Kind::String => {
                let payload = self.string_payload(node_id);
                Some(Rc::new((
                    [self.string_value(payload.template).to_owned()].into(),
                    SyntaxSet::EMPTY,
                )))
            }
            Kind::MultiString => {
                let payload = self.multi_string_payload(node_id);
                let raws = self
                    .kids_slice(payload.templates)
                    .iter()
                    .map(|id| self.string_value(id.0).to_owned())
                    .collect();
                Some(Rc::new((raws, SyntaxSet::EMPTY)))
            }
            Kind::Typed => {
                let payload = self.typed_payload(node_id);
                Some(Rc::new((
                    AHashSet::new(),
                    SyntaxSet::new(&[payload.template]),
                )))
            }
            Kind::NodeMatcher => {
                let payload = self.node_matcher_payload(node_id);
                self.simple(payload.child, parse_context, crumbs)
            }
            Kind::Bracketed => {
                let payload = self.bracketed_payload(node_id);
                let set = self.symbol_name(payload.bracket_pairs_set);
                let target_type = self.symbol_name(payload.bracket_type);
                let mut start = None;

                for (bracket_type, start_ref, _end_ref, _persists) in
                    parse_context.dialect.bracket_sets(set)
                {
                    if bracket_type == target_type
                        && let Some(definition) = self.get_definition_by_name(start_ref)
                    {
                        start = Some(definition);
                        break;
                    }
                }

                start.and_then(|it| self.simple(it, parse_context, crumbs))
            }
            _ => None,
        };

        if cacheable {
            parse_context.simple_cache.insert(node_id, result.clone());
        }

        result
    }

    fn is_optional(&self, node_id: NodeId) -> bool {
        match self.node(node_id).kind {
            Kind::Sequence => self.sequence_payload(node_id).optional,
            Kind::OneOf | Kind::AnyNumberOf => {
                let payload = self.any_number_of_payload(node_id);
                payload.optional || payload.min_times == 0
            }
            Kind::Ref => self.ref_payload(node_id).optional,
            Kind::String => self.string_payload(node_id).optional,
            Kind::Typed => self.typed_payload(node_id).optional,
            Kind::Delimited => self.delimited_payload(node_id).optional,
            Kind::Bracketed => self.is_optional(self.bracketed_payload(node_id).inner),
            _ => false,
        }
    }

    fn match_node(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        match self.node(node_id).kind {
            Kind::Sequence => self.match_sequence(node_id, segments, idx, parse_context),
            Kind::OneOf | Kind::AnyNumberOf => {
                self.match_any_number_of(node_id, segments, idx, parse_context)
            }
            Kind::Ref => self.match_ref(node_id, segments, idx, parse_context),
            Kind::NodeMatcher => self.match_node_matcher(node_id, segments, idx, parse_context),
            Kind::String => self.match_string(node_id, segments, idx),
            Kind::MultiString => self.match_multi_string(node_id, segments, idx),
            Kind::Regex => self.match_regex(node_id, segments, idx),
            Kind::Typed => self.match_typed(node_id, segments, idx),
            Kind::Code => self.match_code(segments, idx),
            Kind::NonCode => self.match_non_code(segments, idx),
            Kind::Nothing => Ok(MatchResult::empty_at(idx)),
            Kind::Anything => self.match_anything(node_id, segments, idx, parse_context),
            Kind::Delimited => self.match_delimited(node_id, segments, idx, parse_context),
            Kind::Bracketed => self.match_bracketed(node_id, segments, idx, parse_context),
            Kind::Meta => panic!("Meta node has no direct match method"),
            Kind::Conditional => self.match_conditional(node_id, idx, parse_context),
            Kind::BracketedSegmentMatcher => self.match_bracketed_segment(segments, idx),
            Kind::LookaheadExclude => self.match_lookahead_exclude(node_id, segments, idx),
        }
    }

    fn match_sequence(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        mut idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.sequence_payload(node_id);
        let children = self.node_children(node_id);

        let start_idx = idx;
        let mut matched_idx = idx;
        let mut max_idx = segments.len() as u32;
        let mut insert_segments = Vec::new();
        let mut child_matches = Vec::new();
        let mut first_match = true;
        let mut meta_buffer = Vec::new();

        if payload.parse_mode == ParseMode::Greedy {
            let payload_terminators = self.kids_slice(payload.terminators);
            let mut terminators =
                Vec::with_capacity(payload_terminators.len() + parse_context.terminators.len());
            terminators.extend_from_slice(payload_terminators);
            terminators.extend_from_slice(&parse_context.terminators);

            max_idx = self.trim_to_terminator(segments, idx, &terminators, parse_context)?;
        }

        for child in children {
            match self.node(*child).kind {
                Kind::Conditional => {
                    let match_result =
                        self.match_node(*child, segments, matched_idx, parse_context)?;
                    for (_, submatch) in match_result.insert_segments {
                        meta_buffer.push(submatch);
                    }
                    continue;
                }
                Kind::Meta => {
                    meta_buffer.push(self.meta_payload(*child).kind);
                    continue;
                }
                _ => {}
            }

            idx = if payload.allow_gaps {
                skip_start_index_forward_to_code(segments, matched_idx, max_idx)
            } else {
                matched_idx
            };

            if idx >= max_idx {
                if self.is_optional(*child) {
                    continue;
                }

                if payload.parse_mode == ParseMode::Strict || matched_idx == start_idx {
                    return Ok(MatchResult::empty_at(idx));
                }

                insert_segments.extend(meta_buffer.into_iter().map(|meta| (matched_idx, meta)));

                return Ok(MatchResult {
                    span: Span {
                        start: start_idx,
                        end: matched_idx,
                    },
                    insert_segments,
                    child_matches,
                    matched: Some(Matched::SyntaxKind(SyntaxKind::Unparsable)),
                });
            }

            let mut elem_match = parse_context.deeper_match(false, &[], |ctx| {
                self.match_node(*child, &segments[..max_idx as usize], idx, ctx)
            })?;

            if !elem_match.has_match() {
                if self.is_optional(*child) {
                    continue;
                }

                if payload.parse_mode == ParseMode::Strict {
                    return Ok(MatchResult::empty_at(idx));
                }

                if payload.parse_mode == ParseMode::GreedyOnceStarted && matched_idx == start_idx {
                    return Ok(MatchResult::empty_at(idx));
                }

                if matched_idx == start_idx {
                    return Ok(MatchResult {
                        span: Span {
                            start: start_idx,
                            end: max_idx,
                        },
                        matched: Some(Matched::SyntaxKind(SyntaxKind::Unparsable)),
                        ..MatchResult::default()
                    });
                }

                child_matches.push(MatchResult {
                    span: Span {
                        start: skip_start_index_forward_to_code(segments, matched_idx, max_idx),
                        end: max_idx,
                    },
                    matched: Some(Matched::SyntaxKind(SyntaxKind::Unparsable)),
                    ..MatchResult::default()
                });

                return Ok(MatchResult {
                    span: Span {
                        start: start_idx,
                        end: max_idx,
                    },
                    insert_segments,
                    child_matches,
                    matched: None,
                });
            }

            let meta = std::mem::take(&mut meta_buffer);
            insert_segments.append(&mut flush_metas(matched_idx, idx, meta));

            matched_idx = elem_match.span.end;

            if first_match && payload.parse_mode == ParseMode::GreedyOnceStarted {
                let payload_terminators = self.kids_slice(payload.terminators);
                let mut terminators =
                    Vec::with_capacity(payload_terminators.len() + parse_context.terminators.len());
                terminators.extend_from_slice(payload_terminators);
                terminators.extend_from_slice(&parse_context.terminators);

                max_idx =
                    self.trim_to_terminator(segments, matched_idx, &terminators, parse_context)?;
                first_match = false;
            }

            if elem_match.matched.is_some() {
                child_matches.push(elem_match);
                continue;
            }

            child_matches.append(&mut elem_match.child_matches);
            insert_segments.append(&mut elem_match.insert_segments);
        }

        insert_segments.extend(meta_buffer.into_iter().map(|meta| (matched_idx, meta)));

        if matches!(
            payload.parse_mode,
            ParseMode::Greedy | ParseMode::GreedyOnceStarted
        ) && max_idx > matched_idx
        {
            let idx = skip_start_index_forward_to_code(segments, matched_idx, max_idx);
            let stop_idx = skip_stop_index_backward_to_code(segments, max_idx, idx);

            if stop_idx > idx {
                child_matches.push(MatchResult {
                    span: Span {
                        start: idx,
                        end: stop_idx,
                    },
                    matched: Some(Matched::SyntaxKind(SyntaxKind::Unparsable)),
                    ..Default::default()
                });
                matched_idx = stop_idx;
            }
        }

        Ok(MatchResult {
            span: Span {
                start: start_idx,
                end: matched_idx,
            },
            matched: None,
            insert_segments,
            child_matches,
        })
    }

    fn match_any_number_of(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.any_number_of_payload(node_id);
        let elements = self.node_children(node_id);

        if let Some(exclude) = payload.exclude {
            let match_result = parse_context.deeper_match(false, &[], |ctx| {
                self.match_node(exclude, segments, idx, ctx)
            })?;

            if match_result.has_match() {
                return Ok(MatchResult::empty_at(idx));
            }
        }

        let mut n_matches = 0;
        let mut option_counter: Option<AHashMap<NodeId, usize>> = payload
            .max_times_per_element
            .map(|_| elements.iter().copied().map(|elem| (elem, 0)).collect());
        let mut matched_idx = idx;
        let mut working_idx = idx;
        let mut matched = MatchResult::empty_at(idx);
        let mut max_idx = segments.len() as u32;

        if payload.parse_mode == ParseMode::Greedy {
            let payload_terminators = self.kids_slice(payload.terminators);
            let mut terminators = if payload.reset_terminators {
                Vec::with_capacity(payload_terminators.len())
            } else {
                Vec::with_capacity(payload_terminators.len() + parse_context.terminators.len())
            };
            terminators.extend_from_slice(payload_terminators);
            if !payload.reset_terminators {
                terminators.extend_from_slice(&parse_context.terminators);
            }

            max_idx = self.trim_to_terminator(segments, idx, &terminators, parse_context)?;
        }

        loop {
            if (n_matches >= payload.min_times && matched_idx >= max_idx)
                || payload.max_times.is_some() && Some(n_matches) >= payload.max_times
            {
                return Ok(parse_mode_match_result(
                    segments,
                    matched,
                    max_idx,
                    payload.parse_mode,
                ));
            }

            if matched_idx >= max_idx {
                return Ok(MatchResult::empty_at(idx));
            }

            let (match_result, matched_option) = parse_context.deeper_match(
                payload.reset_terminators,
                self.kids_slice(payload.terminators),
                |ctx| self.longest_match(&segments[..max_idx as usize], elements, working_idx, ctx),
            )?;

            if !match_result.has_match() {
                if n_matches < payload.min_times {
                    matched = MatchResult::empty_at(idx);
                }

                return Ok(parse_mode_match_result(
                    segments,
                    matched,
                    max_idx,
                    payload.parse_mode,
                ));
            }

            let matched_option = matched_option.unwrap();

            if let Some(max_times_per_element) = payload.max_times_per_element
                && let Some(counter) = option_counter
                    .as_mut()
                    .and_then(|counter| counter.get_mut(&matched_option))
            {
                *counter += 1;

                if *counter > max_times_per_element {
                    return Ok(parse_mode_match_result(
                        segments,
                        matched,
                        max_idx,
                        payload.parse_mode,
                    ));
                }
            }

            matched = matched.append(match_result);
            matched_idx = matched.span.end;
            working_idx = matched_idx;
            if payload.allow_gaps {
                working_idx =
                    skip_start_index_forward_to_code(segments, matched_idx, segments.len() as u32);
            }
            n_matches += 1;
        }
    }

    fn match_ref(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.ref_payload(node_id);
        let Some(elem) = payload.resolved else {
            return Err(SQLParseError {
                description: format!(
                    "Grammar refers to '{}' which was not found in the compiled grammar.",
                    self.symbol_name(payload.symbol)
                ),
                segment: segments.get(idx as usize).cloned(),
            });
        };

        if let Some(exclude) = payload.exclude {
            let ctx = parse_context.deeper_match(
                payload.reset_terminators,
                self.kids_slice(payload.terminators),
                |this| {
                    if self
                        .match_node(exclude, segments, idx, this)
                        .inspect_err(|e| log::error!("Parser error: {e:?}"))
                        .is_ok_and(|match_result| match_result.has_match())
                    {
                        return Some(MatchResult::empty_at(idx));
                    }

                    None
                },
            );

            if let Some(ctx) = ctx {
                return Ok(ctx);
            }
        }

        parse_context.deeper_match(
            payload.reset_terminators,
            self.kids_slice(payload.terminators),
            |this| self.match_node(elem, segments, idx, this),
        )
    }

    fn match_node_matcher(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if idx >= segments.len() as u32 {
            return Ok(MatchResult::empty_at(idx));
        }

        let payload = self.node_matcher_payload(node_id);

        if segments[idx as usize].get_type() == payload.node_kind {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        let match_result = parse_context.deeper_match(false, &[], |ctx| {
            self.match_node(payload.child, segments, idx, ctx)
        })?;

        Ok(match_result.wrap(Matched::SyntaxKind(payload.node_kind)))
    }

    fn match_string(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.string_payload(node_id);
        let segment = &segments[idx as usize];

        if segment.is_code()
            && self
                .string_value(payload.template)
                .eq_ignore_ascii_case(segment.raw())
        {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Some(Matched::Newtype(payload.kind)),
                insert_segments: Vec::new(),
                child_matches: Vec::new(),
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_multi_string(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.multi_string_payload(node_id);
        let segment = &segments[idx as usize];

        if !segment.is_code() {
            return Ok(MatchResult::empty_at(idx));
        }

        let segment_raw = segment.raw();

        let matched = self.kids_slice(payload.templates).iter().any(|template| {
            self.string_value(template.0)
                .eq_ignore_ascii_case(segment_raw)
        });

        if matched {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Some(Matched::Newtype(payload.kind)),
                ..<_>::default()
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_regex(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.regex_payload(node_id);
        let regex = &self.regexes[payload.regex_id as usize];
        let segment = &segments[idx as usize];
        let segment_raw_upper = segment.raw().to_ascii_uppercase();

        if let Some(result) = regex.regex.find(&segment_raw_upper).ok().flatten()
            && result.as_str() == segment_raw_upper.as_str()
            && !regex.anti_regex.as_ref().is_some_and(|anti_template| {
                anti_template
                    .is_match(&segment_raw_upper)
                    .unwrap_or_default()
            })
        {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Some(Matched::Newtype(payload.kind)),
                insert_segments: Vec::new(),
                child_matches: Vec::new(),
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_typed(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.typed_payload(node_id);
        let segment = &segments[idx as usize];
        if segment.is_type(payload.template) {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                matched: Some(Matched::Newtype(payload.kind)),
                insert_segments: Vec::new(),
                child_matches: Vec::new(),
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_code(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        if idx as usize >= segments.len() {
            return Ok(MatchResult::empty_at(idx));
        }

        if segments[idx as usize].is_code() {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_non_code(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let mut matched_idx = idx;

        for i in idx..segments.len() as u32 {
            if segments[i as usize].is_code() {
                matched_idx = i;
                break;
            }
        }

        if matched_idx > idx {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: matched_idx,
                },
                ..Default::default()
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_anything(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.anything_payload(node_id);
        if payload.terminators.is_empty() && parse_context.terminators.is_empty() {
            return Ok(MatchResult::from_span(idx, segments.len() as u32));
        }

        let mut terminators = self.kids_slice(payload.terminators).to_vec();
        terminators.extend_from_slice(&parse_context.terminators);

        self.greedy_match(segments, idx, parse_context, &terminators, false, true)
    }

    fn match_delimited(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.delimited_payload(node_id);
        let elements = self.node_children(node_id);

        let mut delimiters = 0;
        let mut seeking_delimiter = false;
        let max_idx = segments.len() as u32;
        let mut working_idx = idx;
        let mut working_match = MatchResult::empty_at(idx);
        let mut delimiter_match = None;

        let delimiter_matcher = payload.delimiter;

        let mut terminator_matchers = self.kids_slice(payload.terminators).to_vec();
        terminator_matchers.extend(
            parse_context
                .terminators
                .iter()
                .filter(|&&t| self.node_eq_group(delimiter_matcher) != self.node_eq_group(t))
                .copied(),
        );

        let delimiter_matchers = [payload.delimiter];

        if !payload.allow_gaps {
            terminator_matchers.push(self.builtin_non_code);
        }

        loop {
            if payload.allow_gaps && working_idx > idx {
                working_idx =
                    skip_start_index_forward_to_code(segments, working_idx, segments.len() as u32);
            }

            if working_idx >= max_idx {
                break;
            }

            let (match_result, _) = parse_context.deeper_match(false, &[], |this| {
                self.longest_match(segments, &terminator_matchers, working_idx, this)
            })?;

            if match_result.has_match() {
                break;
            }

            let mut push_terminators: &[NodeId] = &[];
            if !seeking_delimiter {
                push_terminators = &delimiter_matchers;
            }

            let (match_result, _) =
                parse_context.deeper_match(false, push_terminators, |this| {
                    self.longest_match(
                        segments,
                        if seeking_delimiter {
                            &delimiter_matchers
                        } else {
                            elements
                        },
                        working_idx,
                        this,
                    )
                })?;

            if !match_result.has_match() {
                if seeking_delimiter && payload.optional_delimiter {
                    seeking_delimiter = false;
                    continue;
                }
                break;
            }

            working_idx = match_result.span.end;

            if seeking_delimiter {
                delimiter_match = Some(match_result);
            } else {
                if let Some(delimiter_match) = &delimiter_match {
                    delimiters += 1;
                    working_match = working_match.append(delimiter_match);
                }
                working_match = working_match.append(match_result);
            }

            seeking_delimiter = !seeking_delimiter;
        }

        if let Some(delimiter_match) =
            delimiter_match.filter(|_delimiter_match| payload.allow_trailing && !seeking_delimiter)
        {
            delimiters += 1;
            working_match = working_match.append(delimiter_match);
        }

        if delimiters < payload.min_delimiters {
            return Ok(MatchResult::empty_at(idx));
        }

        Ok(working_match)
    }

    fn match_bracketed(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.bracketed_payload(node_id);
        let set_name = self.symbol_name(payload.bracket_pairs_set);
        let target_type = self.symbol_name(payload.bracket_type);

        let Some((start_bracket, end_bracket, bracket_persists)) = parse_context
            .dialect
            .bracket_sets(set_name)
            .into_iter()
            .find_map(|(bracket_type, start_ref, end_ref, persists)| {
                if bracket_type != target_type {
                    return None;
                }

                Some((
                    self.get_definition_by_name(start_ref)?,
                    self.get_definition_by_name(end_ref)?,
                    persists,
                ))
            })
        else {
            panic!(
                "bracket_type {:?} not found in bracket_pairs ({set_name}) of {:?} dialect.",
                target_type, parse_context.dialect.name
            );
        };

        let start_match = parse_context.deeper_match(false, &[], |ctx| {
            self.match_node(start_bracket, segments, idx, ctx)
        })?;

        if !start_match.has_match() {
            return Ok(MatchResult::empty_at(idx));
        }

        let start_match_span = start_match.span;

        let bracketed_match = self.resolve_bracket(
            segments,
            start_match,
            start_bracket,
            &[start_bracket],
            &[end_bracket],
            &[bracket_persists],
            parse_context,
            false,
        )?;

        let mut idx = start_match_span.end;
        let mut end_idx = bracketed_match.span.end - 1;

        if payload.allow_gaps {
            idx = skip_start_index_forward_to_code(segments, idx, segments.len() as u32);
            end_idx = skip_stop_index_backward_to_code(segments, end_idx, idx);
        }

        let content_match = parse_context.deeper_match(true, &[end_bracket], |ctx| {
            self.match_node(payload.inner, &segments[..end_idx as usize], idx, ctx)
        })?;

        if content_match.span.end != end_idx && payload.parse_mode == ParseMode::Strict {
            return Ok(MatchResult::empty_at(idx));
        }

        let intermediate_slice = Span {
            start: content_match.span.end,
            end: bracketed_match.span.end - 1,
        };

        if !payload.allow_gaps && intermediate_slice.start == intermediate_slice.end {
            unimplemented!()
        }

        let mut child_matches = bracketed_match.child_matches;
        if content_match.matched.is_some() {
            child_matches.push(content_match);
        } else {
            child_matches.extend(content_match.child_matches);
        }

        Ok(MatchResult {
            child_matches,
            ..bracketed_match
        })
    }

    fn match_conditional(
        &self,
        node_id: NodeId,
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.conditional_payload(node_id);
        if !parse_context
            .indentation_config
            .contains(payload.requirements)
        {
            return Ok(MatchResult::empty_at(idx));
        }

        Ok(MatchResult {
            span: Span {
                start: idx,
                end: idx,
            },
            insert_segments: vec![(idx, payload.meta)],
            ..Default::default()
        })
    }

    fn match_bracketed_segment(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        if segments[idx as usize].get_type() == SyntaxKind::Bracketed {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn match_lookahead_exclude(
        &self,
        node_id: NodeId,
        segments: &[ErasedSegment],
        idx: u32,
    ) -> Result<MatchResult, SQLParseError> {
        let payload = self.lookahead_payload(node_id);

        if idx >= segments.len() as u32 {
            return Ok(MatchResult::empty_at(idx));
        }

        let current_raw = segments[idx as usize].raw();
        if current_raw.eq_ignore_ascii_case(self.string_value(payload.first_token)) {
            let next_idx =
                skip_start_index_forward_to_code(segments, idx + 1, segments.len() as u32);

            if next_idx < segments.len() as u32 {
                let next_raw = segments[next_idx as usize].raw();
                if next_raw.eq_ignore_ascii_case(self.string_value(payload.lookahead_token)) {
                    return Ok(MatchResult::from_span(idx, idx + 1));
                }
            }
        }

        Ok(MatchResult::empty_at(idx))
    }

    #[inline]
    fn option_matches_first_token(
        &self,
        option: NodeId,
        parse_context: &mut CompiledParseContext,
        first_token: Option<(&str, &SyntaxSet)>,
    ) -> bool {
        let Some((first_raw, first_types)) = first_token else {
            return true;
        };

        let Some(simple) = self.simple(option, parse_context, None) else {
            return true;
        };
        let (simple_raws, simple_types) = simple.as_ref();
        simple_raws.contains(first_raw) || first_types.intersects(simple_types)
    }

    fn longest_match(
        &self,
        segments: &[ErasedSegment],
        matchers: &[NodeId],
        idx: u32,
        parse_context: &mut CompiledParseContext,
    ) -> Result<(MatchResult, Option<NodeId>), SQLParseError> {
        let max_idx = segments.len() as u32;

        if matchers.is_empty() || idx == max_idx {
            return Ok((MatchResult::empty_at(idx), None));
        }

        let first_token = first_non_whitespace(segments, idx);
        let first_token = first_token
            .as_ref()
            .map(|(first_raw, first_types)| (first_raw.as_str(), *first_types));
        let mut available_options_count = 0;

        for &matcher in matchers {
            if self.option_matches_first_token(matcher, parse_context, first_token) {
                available_options_count += 1;
            }
        }

        if available_options_count == 0 {
            return Ok((MatchResult::empty_at(idx), None));
        }

        let mut terminators_for_early_break: Option<Vec<NodeId>> = None;
        let cache_position = segments[idx as usize].get_position_marker().unwrap();

        let (working_line_no, working_line_pos) = cache_position.working_loc();
        let loc_key = (
            working_line_no,
            working_line_pos,
            segments[idx as usize].get_type(),
            max_idx,
        );

        let loc_key = parse_context.loc_key(loc_key);

        let mut best_match = MatchResult::empty_at(idx);
        let mut best_matcher = None;
        let mut available_options_seen = 0;

        'matcher: for &matcher in matchers {
            if !self.option_matches_first_token(matcher, parse_context, first_token) {
                continue;
            }

            available_options_seen += 1;
            let matcher_key = self.node_cache_key(matcher);
            let res_match =
                if let Some(res_match) = parse_context.check_parse_cache(loc_key, matcher_key) {
                    res_match
                } else {
                    let computed = self.match_node(matcher, segments, idx, parse_context)?;
                    parse_context.put_parse_cache(loc_key, matcher_key, computed)
                };

            if res_match.has_match() && res_match.span.end == max_idx {
                return Ok((res_match.clone(), Some(matcher)));
            }

            if res_match.is_better_than(&best_match) {
                best_match = res_match.clone();
                best_matcher = Some(matcher);

                if available_options_seen == available_options_count {
                    break 'matcher;
                } else if !parse_context.terminators.is_empty() {
                    let next_code_idx = skip_start_index_forward_to_code(
                        segments,
                        best_match.span.end,
                        segments.len() as u32,
                    );

                    if next_code_idx == segments.len() as u32 {
                        break 'matcher;
                    }

                    let terminators = terminators_for_early_break
                        .get_or_insert_with(|| parse_context.terminators.clone());

                    for terminator in terminators.iter().copied() {
                        let terminator_match =
                            self.match_node(terminator, segments, next_code_idx, parse_context)?;

                        if terminator_match.has_match() {
                            break 'matcher;
                        }
                    }
                }
            }
        }

        Ok((best_match, best_matcher))
    }

    fn prepare_next_match(
        &self,
        matchers: &[NodeId],
        parse_context: &mut CompiledParseContext,
    ) -> NextMatchPrepared {
        let mut raw_simple_map: AHashMap<String, Vec<usize>> = AHashMap::new();
        let mut type_simple_map: AHashMap<SyntaxKind, Vec<usize>> = AHashMap::new();

        for (matcher_idx, matcher) in enumerate(matchers) {
            let Some(simple) = self.simple(*matcher, parse_context, None) else {
                continue;
            };
            let (raws, types) = simple.as_ref();

            raw_simple_map.reserve(raws.len());
            type_simple_map.reserve(types.len());

            for raw in raws {
                raw_simple_map
                    .entry(raw.clone())
                    .or_default()
                    .push(matcher_idx);
            }

            for typ in types {
                type_simple_map.entry(typ).or_default().push(matcher_idx);
            }
        }

        NextMatchPrepared {
            type_simple_keys: type_simple_map.keys().copied().collect(),
            raw_simple_map,
            type_simple_map,
        }
    }

    fn next_match_with_prepared(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        matchers: &[NodeId],
        prepared: &NextMatchPrepared,
        parse_context: &mut CompiledParseContext,
    ) -> Result<(MatchResult, Option<NodeId>), SQLParseError> {
        let max_idx = segments.len() as u32;

        if idx >= max_idx {
            return Ok((MatchResult::empty_at(idx), None));
        }

        let mut matcher_idxs = Vec::new();
        let mut visited = vec![0_u32; matchers.len()];
        let mut visit_stamp = 1_u32;

        for scan_idx in idx..max_idx {
            let seg = &segments[scan_idx as usize];
            matcher_idxs.clear();
            visit_stamp = visit_stamp.wrapping_add(1);
            if visit_stamp == 0 {
                visited.fill(0);
                visit_stamp = 1;
            }

            if let Some(raw_matchers) = prepared.raw_simple_map.get(&first_trimmed_raw(seg)) {
                for &matcher_idx in raw_matchers {
                    if visited[matcher_idx] != visit_stamp {
                        visited[matcher_idx] = visit_stamp;
                        matcher_idxs.push(matcher_idx);
                    }
                }
            }

            let type_overlap = seg
                .class_types()
                .clone()
                .intersection(&prepared.type_simple_keys);

            for typ in type_overlap {
                if let Some(type_matchers) = prepared.type_simple_map.get(&typ) {
                    for &matcher_idx in type_matchers {
                        if visited[matcher_idx] != visit_stamp {
                            visited[matcher_idx] = visit_stamp;
                            matcher_idxs.push(matcher_idx);
                        }
                    }
                }
            }

            if matcher_idxs.is_empty() {
                continue;
            }

            for &matcher_idx in &matcher_idxs {
                let matcher = matchers[matcher_idx];
                let match_result = self.match_node(matcher, segments, scan_idx, parse_context)?;

                if match_result.has_match() {
                    return Ok((match_result, Some(matcher)));
                }
            }
        }

        Ok((MatchResult::empty_at(idx), None))
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_bracket(
        &self,
        segments: &[ErasedSegment],
        opening_match: MatchResult,
        opening_matcher: NodeId,
        start_brackets: &[NodeId],
        end_brackets: &[NodeId],
        bracket_persists: &[bool],
        parse_context: &mut CompiledParseContext,
        nested_match: bool,
    ) -> Result<MatchResult, SQLParseError> {
        let type_idx = start_brackets
            .iter()
            .position(|it| it == &opening_matcher)
            .unwrap();
        let mut matched_idx = opening_match.span.end;
        let mut child_matches = vec![opening_match.clone()];

        let mut matchers = Vec::with_capacity(start_brackets.len() + end_brackets.len());
        matchers.extend_from_slice(start_brackets);
        matchers.extend_from_slice(end_brackets);
        let prepared = self.prepare_next_match(&matchers, parse_context);

        loop {
            let (match_result, matcher) = self.next_match_with_prepared(
                segments,
                matched_idx,
                &matchers,
                &prepared,
                parse_context,
            )?;

            if !match_result.has_match() {
                return Err(SQLParseError {
                    description: "Couldn't find closing bracket for opening bracket.".into(),
                    segment: segments[opening_match.span.start as usize].clone().into(),
                });
            }

            let matcher = matcher.unwrap();
            if end_brackets.contains(&matcher) {
                let closing_idx = end_brackets.iter().position(|it| it == &matcher).unwrap();

                if closing_idx == type_idx {
                    let match_span = match_result.span;
                    let persists = bracket_persists[type_idx];
                    let insert_segments = vec![
                        (opening_match.span.end, SyntaxKind::Indent),
                        (match_result.span.start, SyntaxKind::Dedent),
                    ];

                    child_matches.push(match_result);
                    let match_result = MatchResult {
                        span: Span {
                            start: opening_match.span.start,
                            end: match_span.end,
                        },
                        matched: None,
                        insert_segments,
                        child_matches,
                    };

                    if !persists {
                        return Ok(match_result);
                    }

                    return Ok(match_result.wrap(Matched::SyntaxKind(SyntaxKind::Bracketed)));
                }

                return Err(SQLParseError {
                    description: "Found unexpected end bracket!".into(),
                    segment: segments[(match_result.span.end - 1) as usize]
                        .clone()
                        .into(),
                });
            }

            let inner_match = self.resolve_bracket(
                segments,
                match_result,
                matcher,
                start_brackets,
                end_brackets,
                bracket_persists,
                parse_context,
                false,
            )?;

            matched_idx = inner_match.span.end;
            if nested_match {
                child_matches.push(inner_match);
            }
        }
    }

    fn next_ex_bracket_match(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        matchers: &[NodeId],
        parse_context: &mut CompiledParseContext,
        bracket_data: &NextExBracketPrepared,
    ) -> BracketMatch {
        let max_idx = segments.len() as u32;

        if idx >= max_idx {
            return Ok((MatchResult::empty_at(idx), None, Vec::new()));
        }

        let mut matched_idx = idx;
        let mut child_matches: Vec<MatchResult> = Vec::new();

        loop {
            let (match_result, matcher) = self.next_match_with_prepared(
                segments,
                matched_idx,
                &bracket_data.all_matchers,
                &bracket_data.next_match_prepared,
                parse_context,
            )?;
            if !match_result.has_match() {
                return Ok((match_result, matcher, child_matches));
            }

            if let Some(matcher) = matcher
                .as_ref()
                .filter(|matcher| matchers.contains(matcher))
            {
                return Ok((match_result, Some(*matcher), child_matches));
            }

            if matcher
                .as_ref()
                .is_some_and(|matcher| bracket_data.end_brackets.contains(matcher))
            {
                return Ok((MatchResult::empty_at(idx), None, Vec::new()));
            }

            let bracket_match = self.resolve_bracket(
                segments,
                match_result,
                matcher.unwrap(),
                &bracket_data.start_brackets,
                &bracket_data.end_brackets,
                &bracket_data.bracket_persists,
                parse_context,
                true,
            )?;

            matched_idx = bracket_match.span.end;
            child_matches.push(bracket_match);
        }
    }

    fn prepare_next_ex_bracket_match(
        &self,
        matchers: &[NodeId],
        parse_context: &mut CompiledParseContext,
        bracket_pairs_set: &str,
    ) -> NextExBracketPrepared {
        let (_, start_bracket_refs, end_bracket_refs, bracket_persists): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = multiunzip(parse_context.dialect.bracket_sets(bracket_pairs_set));

        let start_brackets = start_bracket_refs
            .into_iter()
            .filter_map(|seg_ref| self.get_definition_by_name(seg_ref))
            .collect_vec();

        let end_brackets = end_bracket_refs
            .into_iter()
            .filter_map(|seg_ref| self.get_definition_by_name(seg_ref))
            .collect_vec();

        let mut all_matchers =
            Vec::with_capacity(matchers.len() + start_brackets.len() + end_brackets.len());
        all_matchers.extend_from_slice(matchers);
        all_matchers.extend_from_slice(&start_brackets);
        all_matchers.extend_from_slice(&end_brackets);

        let next_match_prepared = self.prepare_next_match(&all_matchers, parse_context);

        NextExBracketPrepared {
            start_brackets,
            end_brackets,
            bracket_persists,
            all_matchers,
            next_match_prepared,
        }
    }

    fn greedy_match(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut CompiledParseContext,
        matchers: &[NodeId],
        include_terminator: bool,
        nested_match: bool,
    ) -> Result<MatchResult, SQLParseError> {
        let mut working_idx = idx;
        let mut stop_idx: u32;
        let mut child_matches = Vec::new();
        let mut matched: MatchResult;
        let bracket_data =
            self.prepare_next_ex_bracket_match(matchers, parse_context, "bracket_pairs");

        loop {
            let (match_result, matcher, inner_matches) =
                parse_context.deeper_match(false, &[], |ctx| {
                    self.next_ex_bracket_match(segments, working_idx, matchers, ctx, &bracket_data)
                })?;

            matched = match_result;

            if nested_match {
                child_matches.extend(inner_matches);
            }

            if !matched.has_match() {
                return Ok(MatchResult {
                    span: Span {
                        start: idx,
                        end: segments.len() as u32,
                    },
                    matched: None,
                    insert_segments: Vec::new(),
                    child_matches,
                });
            }

            let start_idx = matched.span.start;
            stop_idx = matched.span.end;

            let matcher = matcher.unwrap();
            let simple = self.simple(matcher, parse_context, None);

            if let Some(simple) = simple {
                let (strings, types) = simple.as_ref();
                if types.is_empty() && strings.iter().all(|s| s.chars().all(|c| c.is_alphabetic()))
                {
                    let mut allowable_match = start_idx == working_idx;

                    for idx in (working_idx..=start_idx).rev() {
                        if segments[idx as usize - 1].is_meta() {
                            continue;
                        }

                        allowable_match = matches!(
                            segments[idx as usize - 1].get_type(),
                            SyntaxKind::Whitespace | SyntaxKind::Newline
                        );

                        break;
                    }

                    if !allowable_match {
                        working_idx = stop_idx;
                        continue;
                    }
                }
            }

            break;
        }

        if include_terminator {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: stop_idx,
                },
                ..MatchResult::default()
            });
        }

        let stop_idx = skip_stop_index_backward_to_code(segments, matched.span.start, idx);

        let span = if idx == stop_idx {
            Span {
                start: idx,
                end: matched.span.start,
            }
        } else {
            Span {
                start: idx,
                end: stop_idx,
            }
        };

        Ok(MatchResult {
            span,
            child_matches,
            ..Default::default()
        })
    }

    fn trim_to_terminator(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        terminators: &[NodeId],
        parse_context: &mut CompiledParseContext,
    ) -> Result<u32, SQLParseError> {
        if idx >= segments.len() as u32 {
            return Ok(segments.len() as u32);
        }

        let first_token = first_non_whitespace(segments, idx);
        let first_token = first_token
            .as_ref()
            .map(|(first_raw, first_types)| (first_raw.as_str(), *first_types));

        let early_return = parse_context.deeper_match(false, &[], |ctx| {
            for &term in terminators {
                if !self.option_matches_first_token(term, ctx, first_token) {
                    continue;
                }
                if self.match_node(term, segments, idx, ctx)?.has_match() {
                    return Ok(Some(idx));
                }
            }

            Ok(None)
        })?;

        if let Some(idx) = early_return {
            return Ok(idx);
        }

        let term_match = parse_context.deeper_match(false, &[], |ctx| {
            self.greedy_match(segments, idx, ctx, terminators, false, false)
        })?;

        Ok(skip_stop_index_backward_to_code(
            segments,
            term_match.span.end,
            idx,
        ))
    }
}

trait PayloadExt {
    fn as_sequence(&self) -> Option<&SequencePayload>;
    fn as_any_number_of(&self) -> Option<&AnyNumberOfPayload>;
}

impl PayloadExt for Payload {
    fn as_sequence(&self) -> Option<&SequencePayload> {
        match self {
            Payload::Sequence(payload) => Some(payload),
            _ => None,
        }
    }

    fn as_any_number_of(&self) -> Option<&AnyNumberOfPayload> {
        match self {
            Payload::AnyNumberOf(payload) => Some(payload),
            _ => None,
        }
    }
}

struct LegacyCompiler<'a> {
    dialect: &'a Dialect,
    grammar: CompiledGrammar,
    seen: AHashMap<usize, NodeId>,
    eq_representatives: Vec<(Matchable, u32)>,
    next_eq_group: u32,
}

impl LegacyCompiler<'_> {
    fn next_eq_group(&mut self) -> u32 {
        let group = self.next_eq_group;
        self.next_eq_group = self.next_eq_group.saturating_add(1);
        group
    }

    fn legacy_eq_group(&mut self, matchable: &Matchable) -> u32 {
        for (representative, group) in &self.eq_representatives {
            if representative == matchable {
                return *group;
            }
        }

        let group = self.next_eq_group();
        self.eq_representatives.push((matchable.clone(), group));
        group
    }

    fn compile_many(&mut self, elems: &[Matchable]) -> Result<Vec<NodeId>, CompileError> {
        elems
            .iter()
            .map(|elem| self.compile_matchable(elem))
            .collect()
    }

    fn compile_slice(&mut self, elems: &[Matchable]) -> Result<NodeSlice, CompileError> {
        let compiled = self.compile_many(elems)?;
        Ok(self.grammar.push_children(compiled))
    }

    fn compile_matchable(&mut self, matchable: &Matchable) -> Result<NodeId, CompileError> {
        let ptr = matchable.ptr() as usize;
        if let Some(id) = self.seen.get(&ptr).copied() {
            return Ok(id);
        }

        let eq_group = self.legacy_eq_group(matchable);

        let id = match matchable.deref() {
            MatchableTraitImpl::AnyNumberOf(any) => {
                let children = self.compile_many(any.elements())?;
                let payload = AnyNumberOfPayload {
                    exclude: any
                        .exclude
                        .as_ref()
                        .map(|exclude| self.compile_matchable(exclude))
                        .transpose()?,
                    terminators: self.compile_slice(&any.terminators)?,
                    reset_terminators: any.reset_terminators,
                    max_times: any.max_times,
                    min_times: any.min_times,
                    max_times_per_element: any.max_times_per_element,
                    allow_gaps: any.allow_gaps,
                    optional: any.is_optional(),
                    parse_mode: any.parse_mode.into(),
                };

                let kind = if any.max_times == Some(1)
                    && any.min_times == 1
                    && any.max_times_per_element.is_none()
                {
                    Kind::OneOf
                } else {
                    Kind::AnyNumberOf
                };

                self.grammar.make_any_number_of(kind, children, payload)
            }
            MatchableTraitImpl::Bracketed(bracketed) => {
                let inner_children = self.compile_many(bracketed.elements())?;
                let inner_terminators = self.compile_many(&bracketed.terminators)?;
                let inner = self.grammar.make_sequence(
                    inner_children,
                    bracketed.parse_mode.into(),
                    bracketed.this.allow_gaps,
                    bracketed.is_optional(),
                    inner_terminators,
                );

                let payload = BracketedPayload {
                    bracket_type: self.grammar.intern_symbol(bracketed.bracket_type),
                    bracket_pairs_set: self.grammar.intern_symbol(bracketed.bracket_pairs_set),
                    allow_gaps: bracketed.outer_allow_gaps(),
                    parse_mode: bracketed.parse_mode.into(),
                    inner,
                };

                self.grammar
                    .push_node(Kind::Bracketed, inner.0, 0, Payload::Bracketed(payload))
            }
            MatchableTraitImpl::NodeMatcher(node_matcher) => {
                let child = self.compile_matchable(&node_matcher.match_grammar(self.dialect))?;
                let payload = NodeMatcherPayload {
                    node_kind: node_matcher.get_type(),
                    child,
                };

                self.grammar.push_node(
                    Kind::NodeMatcher,
                    payload.node_kind as u32,
                    payload.child.0,
                    Payload::NodeMatcher(payload),
                )
            }
            MatchableTraitImpl::NonCodeMatcher(_) => self.grammar.non_code(),
            MatchableTraitImpl::Nothing(_) => self.grammar.nothing(),
            MatchableTraitImpl::Ref(r#ref) => {
                let symbol = self.grammar.intern_symbol(r#ref.reference());
                let payload = RefPayload {
                    symbol,
                    exclude: r#ref
                        .exclude
                        .as_ref()
                        .map(|exclude| self.compile_matchable(exclude))
                        .transpose()?,
                    terminators: self.compile_slice(r#ref.terminators_slice())?,
                    reset_terminators: r#ref.reset_terminators_flag(),
                    optional: r#ref.is_optional(),
                    resolved: None,
                };

                self.grammar
                    .push_node(Kind::Ref, symbol, 0, Payload::Ref(payload))
            }
            MatchableTraitImpl::Sequence(sequence) => {
                let sequence_children = self.compile_many(sequence.elements())?;
                let sequence_terminators = self.compile_many(&sequence.terminators)?;
                self.grammar.make_sequence(
                    sequence_children,
                    sequence.parse_mode.into(),
                    sequence.allow_gaps,
                    sequence.is_optional(),
                    sequence_terminators,
                )
            }
            MatchableTraitImpl::StringParser(parser) => {
                let payload = StringPayload {
                    template: self.grammar.intern_string(parser.template()),
                    kind: parser.kind(),
                    optional: parser.is_optional(),
                };

                self.grammar.push_node(
                    Kind::String,
                    payload.template,
                    payload.kind as u32,
                    Payload::String(payload),
                )
            }
            MatchableTraitImpl::TypedParser(parser) => {
                let payload = TypedPayload {
                    template: parser.template(),
                    kind: parser.kind(),
                    optional: parser.is_optional(),
                };

                self.grammar.push_node(
                    Kind::Typed,
                    payload.template as u32,
                    payload.kind as u32,
                    Payload::Typed(payload),
                )
            }
            MatchableTraitImpl::CodeParser(_) => self.grammar.code(),
            MatchableTraitImpl::MetaSegment(meta) => self.grammar.push_node(
                Kind::Meta,
                meta.kind as u32,
                0,
                Payload::Meta(MetaPayload { kind: meta.kind }),
            ),
            MatchableTraitImpl::MultiStringParser(parser) => {
                let template_ids = parser
                    .templates()
                    .iter()
                    .map(|it| NodeId(self.grammar.intern_string(it)))
                    .collect_vec();
                let template_slice = self.grammar.push_children(template_ids);
                let payload = MultiStringPayload {
                    templates: template_slice,
                    kind: parser.kind(),
                };

                self.grammar.push_node(
                    Kind::MultiString,
                    template_slice.start,
                    template_slice.len,
                    Payload::MultiString(payload),
                )
            }
            MatchableTraitImpl::RegexParser(parser) => {
                let pattern = parser.template.as_str();
                let anti_pattern = parser.anti_template.as_ref().map(|it| it.as_str());
                let regex_id = self.grammar.intern_regex(pattern, anti_pattern);
                let payload = RegexPayload {
                    regex_id,
                    kind: parser.kind(),
                };

                self.grammar.push_node(
                    Kind::Regex,
                    regex_id,
                    payload.kind as u32,
                    Payload::Regex(payload),
                )
            }
            MatchableTraitImpl::Delimited(delimited) => {
                let delimited_children = self.compile_many(delimited.elements())?;
                let child_slice = self.grammar.push_children(delimited_children);
                let payload = DelimitedPayload {
                    allow_trailing: delimited.allow_trailing,
                    delimiter: self.compile_matchable(&delimited.delimiter)?,
                    min_delimiters: delimited.min_delimiters,
                    optional_delimiter: delimited.optional_delimiter,
                    optional: delimited.is_optional(),
                    allow_gaps: delimited.allow_gaps,
                    terminators: self.compile_slice(&delimited.terminators)?,
                };

                self.grammar.push_node(
                    Kind::Delimited,
                    child_slice.start,
                    child_slice.len,
                    Payload::Delimited(payload),
                )
            }
            MatchableTraitImpl::Anything(anything) => {
                let payload = AnythingPayload {
                    terminators: self.compile_slice(anything.terminators_slice())?,
                };
                self.grammar
                    .push_node(Kind::Anything, 0, 0, Payload::Anything(payload))
            }
            MatchableTraitImpl::Conditional(conditional) => {
                let payload = ConditionalPayload {
                    meta: conditional.meta_kind(),
                    requirements: conditional.requirements(),
                };

                self.grammar.push_node(
                    Kind::Conditional,
                    payload.meta as u32,
                    0,
                    Payload::Conditional(payload),
                )
            }
            MatchableTraitImpl::BracketedSegmentMatcher(_) => {
                self.grammar
                    .push_node(Kind::BracketedSegmentMatcher, 0, 0, Payload::None)
            }
            MatchableTraitImpl::LookaheadExclude(lookahead) => {
                let payload = LookaheadExcludePayload {
                    first_token: self.grammar.intern_string(lookahead.first_token()),
                    lookahead_token: self.grammar.intern_string(lookahead.lookahead_token()),
                };

                self.grammar.push_node(
                    Kind::LookaheadExclude,
                    payload.first_token,
                    payload.lookahead_token,
                    Payload::LookaheadExclude(payload),
                )
            }
        };

        self.grammar.set_node_eq_group(id, eq_group);
        self.seen.insert(ptr, id);
        Ok(id)
    }
}

fn parse_mode_match_result(
    segments: &[ErasedSegment],
    current_match: MatchResult,
    max_idx: u32,
    parse_mode: ParseMode,
) -> MatchResult {
    if parse_mode == ParseMode::Strict {
        return current_match;
    }

    let stop_idx = current_match.span.end;
    if stop_idx == max_idx
        || segments[stop_idx as usize..max_idx as usize]
            .iter()
            .all(|it| !it.is_code())
    {
        return current_match;
    }

    let trim_idx = skip_start_index_forward_to_code(segments, stop_idx, segments.len() as u32);

    let unmatched_match = MatchResult {
        span: Span {
            start: trim_idx,
            end: max_idx,
        },
        matched: Some(Matched::SyntaxKind(SyntaxKind::Unparsable)),
        ..MatchResult::default()
    };

    current_match.append(unmatched_match)
}

fn flush_metas(
    tpre_nc_idx: u32,
    post_nc_idx: u32,
    meta_buffer: Vec<SyntaxKind>,
) -> Vec<(u32, SyntaxKind)> {
    let meta_idx = if meta_buffer.iter().all(|it| it.indent_val() >= 0) {
        tpre_nc_idx
    } else {
        post_nc_idx
    };
    meta_buffer.into_iter().map(|it| (meta_idx, it)).collect()
}

impl SQLParseError {
    fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            segment: None,
        }
    }
}

pub type Grammar = CompiledGrammar;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_define_and_compile_resolves_refs() {
        let mut g = CompiledGrammar::new();
        let div = g.keyword("DIV");
        let div_op = g.node_matcher(SyntaxKind::BinaryOperator, div);
        g.define("DivBinaryOperatorSegment", div_op);

        let plus = g.ref_("PlusSegment");
        let minus = g.ref_("MinusSegment");
        let div_ref = g.ref_("DivBinaryOperatorSegment");
        let arith = g.one_of([plus, minus, div_ref]);
        g.define("ArithmeticBinaryOperatorGrammar", arith);

        let plus_kw = g.keyword("+");
        let minus_kw = g.keyword("-");
        g.define("PlusSegment", plus_kw);
        g.define("MinusSegment", minus_kw);

        let g = g.compile().unwrap();
        assert!(g.root("ArithmeticBinaryOperatorGrammar").is_some());
    }
}
