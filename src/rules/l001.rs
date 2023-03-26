use crate::core::rules::base::LintResult;
use crate::core::rules::context::RuleContext;

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
struct RuleL001 {}

impl RuleL001 {
    pub fn groups() -> Vec<&'static str> {
        vec!["all", "core"]
    }

    /// Unnecessary trailing whitespace.
    ///
    /// Look for newline segments, and then evaluate what
    // it was preceded by.
    pub fn _eval(&self, context: RuleContext) -> Option<LintResult> {
        panic!("Not implemented yet.")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rule_l001() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
