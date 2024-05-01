use ahash::AHashSet;
use enum_dispatch::enum_dispatch;
use itertools::{chain, Itertools};

use crate::core::parser::segments::base::Segment;
use crate::core::rules::context::RuleContext;

#[enum_dispatch]
pub trait BaseCrawler {
    fn works_on_unparsable(&self) -> bool {
        false
    }

    fn passes_filter(&self, segment: &dyn Segment) -> bool {
        self.works_on_unparsable() || !segment.is_type("unparsable")
    }

    fn crawl<'a>(&self, context: RuleContext<'a>) -> Vec<RuleContext<'a>>;
}

#[enum_dispatch(BaseCrawler)]
pub enum Crawler {
    RootOnlyCrawler,
    SegmentSeekerCrawler,
}

/// A crawler that doesn't crawl.
///
/// This just yields one context on the root-level (topmost) segment of the
/// file.
#[derive(Debug, Default, Clone)]
pub struct RootOnlyCrawler;

impl BaseCrawler for RootOnlyCrawler {
    fn crawl<'a>(&self, context: RuleContext<'a>) -> Vec<RuleContext<'a>> {
        if self.passes_filter(&*context.segment) { vec![context.clone()] } else { Vec::new() }
    }
}

pub struct SegmentSeekerCrawler {
    types: AHashSet<&'static str>,
    _provide_raw_stack: bool,
    allow_recurse: bool,
}

impl SegmentSeekerCrawler {
    pub fn new(types: AHashSet<&'static str>) -> Self {
        Self { types, _provide_raw_stack: false, allow_recurse: true }
    }

    pub fn disallow_recurse(mut self) -> Self {
        self.allow_recurse = false;
        self
    }

    fn is_self_match(&self, segment: &dyn Segment) -> bool {
        self.types.iter().any(|ty| segment.is_type(ty))
    }
}

impl BaseCrawler for SegmentSeekerCrawler {
    fn crawl<'a>(&self, mut context: RuleContext<'a>) -> Vec<RuleContext<'a>> {
        let mut acc = Vec::new();

        let mut self_match = false;

        if self.is_self_match(&*context.segment) {
            self_match = true;
            acc.push(context.clone());
        }

        if context.segment.segments().is_empty() || (self_match && !self.allow_recurse) {
            return acc;
        }

        self.types.is_disjoint(
            &context.segment.descendant_type_set().iter().map(|it| it.as_str()).collect(),
        );

        let new_parent_stack =
            chain(context.parent_stack, Some(context.segment.clone())).collect_vec();

        #[allow(clippy::assigning_clones)]
        for (idx, child) in context.segment.gather_segments().into_iter().enumerate() {
            context.segment = child;
            context.parent_stack = new_parent_stack.clone();
            context.segment_idx = idx;

            acc.extend(self.crawl(context.clone()));
        }

        acc
    }
}
