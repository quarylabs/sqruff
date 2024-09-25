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

    fn crawl<'a>(&self, context: RuleContext<'a>) -> Vec<RuleContext<'a>>;
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
    fn crawl<'a>(&self, context: RuleContext<'a>) -> Vec<RuleContext<'a>> {
        if self.passes_filter(&context.segment) {
            vec![context.clone()]
        } else {
            Vec::new()
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
    fn crawl<'a>(&self, mut context: RuleContext<'a>) -> Vec<RuleContext<'a>> {
        let mut acc = Vec::new();
        let mut self_match = false;

        if self.is_self_match(&context.segment) {
            self_match = true;
            acc.push(context.clone());
        }

        if context.segment.segments().is_empty() || (self_match && !self.allow_recurse) {
            return acc;
        }

        if !self.types.intersects(context.segment.descendant_type_set()) {
            if self.provide_raw_stack {
                context
                    .raw_stack
                    .append(&mut context.segment.get_raw_segments());
            }

            return acc;
        }

        context.parent_stack.push(context.segment.clone());
        for (idx, child) in context.segment.gather_segments().into_iter().enumerate() {
            context.segment = child;
            context.segment_idx = idx;

            acc.extend(self.crawl(context.clone()));
        }

        acc
    }
}

pub struct TokenSeekerCrawler;

impl BaseCrawler for TokenSeekerCrawler {
    fn crawl<'a>(&self, mut context: RuleContext<'a>) -> Vec<RuleContext<'a>> {
        let mut acc = Vec::new();

        if context.segment.segments().is_empty() {
            acc.push(context.clone());
        }

        context.parent_stack.push(context.segment.clone());
        for (idx, child) in context.segment.gather_segments().into_iter().enumerate() {
            context.segment = child;
            context.segment_idx = idx;

            acc.extend(self.crawl(context.clone()));
        }

        acc
    }
}
