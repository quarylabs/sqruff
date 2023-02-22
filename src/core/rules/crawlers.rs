use crate::core::rules::context::RuleContext;

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
