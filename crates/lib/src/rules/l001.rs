use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

/// Unnecessary trailing whitespace.
///
///     **Anti-pattern**
///     The ``•`` character represents a space.
///     .. code-block:: sql
///        :force:
///         SELECT
///             a
///         FROM foo••
///    **Best practice**
///     Remove trailing spaces.
///     .. code-block:: sql
///         SELECT
///             a
///         FROM foo
#[derive(Default, Debug, Clone)]
pub struct RuleL001 {}

impl Rule for RuleL001 {
    fn from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        unimplemented!()
    }

    fn name(&self) -> &'static str {
        "trailing whitespace"
    }

    fn description(&self) -> &'static str {
        "Unnecessary trailing whitespace."
    }

    /// Unnecessary trailing whitespace.
    ///
    /// Look for newline segments, and then evaluate what
    // it was preceded by.
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let sequence = ReflowSequence::from_root(context.segment, context.config.unwrap());
        sequence.respace(false, Filter::All).results()
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;
    use crate::rules::l001::RuleL001;

    #[test]
    fn test_pass_bigquery_trailing_comma() {
        let sql =
            fix("SELECT * FROM(SELECT 1 AS C1)AS T1;".into(), vec![RuleL001::default().erased()]);
        // FIXME: ` ;` -> `;`
        assert_eq!(sql, "SELECT * FROM (SELECT 1 AS C1) AS T1;");
    }
}
