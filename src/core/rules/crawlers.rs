use crate::core::rules::context::RuleContext;

trait Crawler {
    fn passes_filter(&self) -> bool;
    // yields a RuleContext for each segment the rull should specify
    fn crawl(&self) -> bool;
}

struct BaseCrawler {
    pub works_on_unparsable: bool,
}

impl Crawler for BaseCrawler {
    fn passes_filter(&self) -> bool {
        self.works_on_unparsable
    }
    fn crawl(&self) -> bool {
        true
    }
}

/// A crawler that doesn't crawl.
///
/// This just yields one context on the root-level (topmost) segment of the file.
#[derive(Debug, Clone)]
pub struct RootOnlyCrawler {}

impl RootOnlyCrawler {
    pub fn crawl(&self, context: RuleContext) -> &dyn Iterator<Item = RuleContext> {
        panic!("Not implemented yet.")
    }
}
