use enum_dispatch::enum_dispatch;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::rules::context::RuleContext;

#[enum_dispatch]
pub trait BaseCrawler {
    fn works_on_unparsable(&self) -> bool {
        false
    }

    fn passes_filter(&self, segment: &ErasedSegment) -> bool {
        self.works_on_unparsable() || !segment.is_type(SyntaxKind::Unparsable)
    }

    fn crawl<'a>(&self, context: &mut RuleContext<'a>, f: &mut impl FnMut(&RuleContext<'a>));
}

#[enum_dispatch(BaseCrawler)]
pub enum Crawler {
    RootOnlyCrawler,
    SegmentSeekerCrawler,
    TokenSeekerCrawler,
}

/// A crawler that doesn't crawl.
///
/// This just yields one context on the root-level (topmost) segment of the
/// file.
#[derive(Debug, Default, Clone)]
pub struct RootOnlyCrawler;

impl BaseCrawler for RootOnlyCrawler {
    fn crawl<'a>(&self, context: &mut RuleContext<'a>, f: &mut impl FnMut(&RuleContext<'a>)) {
        if self.passes_filter(&context.segment) {
            f(context);
        }
    }
}

pub struct SegmentSeekerCrawler {
    types: SyntaxSet,
    provide_raw_stack: bool,
    allow_recurse: bool,
}

impl SegmentSeekerCrawler {
    pub fn new(types: SyntaxSet) -> Self {
        Self {
            types,
            provide_raw_stack: false,
            allow_recurse: true,
        }
    }

    pub fn disallow_recurse(mut self) -> Self {
        self.allow_recurse = false;
        self
    }

    pub fn provide_raw_stack(mut self) -> Self {
        self.provide_raw_stack = true;
        self
    }

    fn is_self_match(&self, segment: &ErasedSegment) -> bool {
        self.types.contains(segment.get_type())
    }
}

impl BaseCrawler for SegmentSeekerCrawler {
    fn crawl<'a>(&self, context: &mut RuleContext<'a>, f: &mut impl FnMut(&RuleContext<'a>)) {
        let mut self_match = false;

        if self.is_self_match(&context.segment) {
            self_match = true;
            f(context);
        }

        if context.segment.segments().is_empty() || (self_match && !self.allow_recurse) {
            return;
        }

        if !self.types.intersects(context.segment.descendant_type_set()) {
            if self.provide_raw_stack {
                let raw_segments = context.segment.get_raw_segments();
                context.raw_stack.extend(raw_segments);
            }

            return;
        }

        let segment = context.segment.clone();
        context.parent_stack.push(segment.clone());
        for (idx, child) in segment.segments().iter().enumerate() {
            context.segment = child.clone();
            context.segment_idx = idx;
            let checkpoint = context.checkpoint();
            self.crawl(context, f);
            context.restore(checkpoint);
        }
    }
}

pub struct TokenSeekerCrawler;

impl BaseCrawler for TokenSeekerCrawler {
    fn crawl<'a>(&self, context: &mut RuleContext<'a>, f: &mut impl FnMut(&RuleContext<'a>)) {
        if context.segment.segments().is_empty() {
            f(context);
        }

        let segment = context.segment.clone();
        context.parent_stack.push(segment.clone());
        for (idx, child) in segment.segments().iter().enumerate() {
            context.segment = child.clone();
            context.segment_idx = idx;

            let checkpoint = context.checkpoint();
            self.crawl(context, f);
            context.restore(checkpoint);
        }
    }
}
