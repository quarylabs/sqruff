use std::collections::HashSet;

use itertools::{chain, Itertools};

use crate::core::parser::segments::base::Segment;
use crate::core::rules::context::RuleContext;

pub trait Crawler {
    fn works_on_unparsable(&self) -> bool {
        false
    }

    fn passes_filter(&self, segment: &dyn Segment) -> bool {
        self.works_on_unparsable() || !segment.is_type("unparsable")
    }

    fn crawl(&self, context: RuleContext) -> Vec<RuleContext>;
}

/// A crawler that doesn't crawl.
///
/// This just yields one context on the root-level (topmost) segment of the
/// file.
#[derive(Debug, Default, Clone)]
pub struct RootOnlyCrawler {}

impl Crawler for RootOnlyCrawler {
    fn crawl(&self, context: RuleContext) -> Vec<RuleContext> {
        if self.passes_filter(&*context.segment) { vec![context.clone()] } else { Vec::new() }
    }
}

pub struct SegmentSeekerCrawler {
    types: HashSet<&'static str>,
    provide_raw_stack: bool,
    allow_recurse: bool,
}

impl SegmentSeekerCrawler {
    pub fn new(types: HashSet<&'static str>) -> Self {
        Self { types, provide_raw_stack: false, allow_recurse: true }
    }

    fn is_self_match(&self, segment: &dyn Segment) -> bool {
        self.types.iter().any(|ty| segment.is_type(ty))
    }
}

impl Crawler for SegmentSeekerCrawler {
    fn crawl(&self, mut context: RuleContext) -> Vec<RuleContext> {
        let mut acc = Vec::new();

        let self_match = false;

        if self.is_self_match(&*context.segment) {
            acc.push(context.clone());
        }

        if !context.segment.get_segments().is_empty() && (self_match && !self.allow_recurse) {
            if self.provide_raw_stack {
                unimplemented!();
                return acc;
            }
        }

        if self.types.is_disjoint(
            &context.segment.descendant_type_set().iter().map(|it| it.as_str()).collect(),
        ) {}

        let new_parent_stack =
            chain(context.parent_stack, Some(context.segment.clone())).collect_vec();
        for (idx, child) in context.segment.get_segments().into_iter().enumerate() {
            context.segment = child;
            context.parent_stack = new_parent_stack.clone();
            context.segment_idx = idx;

            acc.extend(self.crawl(context.clone()));
        }

        acc
    }
}
