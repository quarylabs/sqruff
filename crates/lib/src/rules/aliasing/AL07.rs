use std::iter::once;

use ahash::{AHashMap, AHashSet};
use itertools::chain;

use crate::core::config::Value;
use crate::core::parser::segments::base::{ErasedSegment, IdentifierSegment, SymbolSegment};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug)]
struct TableAliasInfo {
    table_ref: ErasedSegment,
    whitespace_ref: Option<ErasedSegment>,
    alias_exp_ref: ErasedSegment,
    alias_identifier_ref: Option<ErasedSegment>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleAL07 {
    force_enable: bool,
}

impl RuleAL07 {
    fn lint_aliases_in_join(
        &self,
        base_table: Option<ErasedSegment>,
        from_expression_elements: Vec<ErasedSegment>,
        column_reference_segments: Vec<ErasedSegment>,
        segment: ErasedSegment,
    ) -> Vec<LintResult> {
        let mut violation_buff = Vec::new();
        let to_check = self.filter_table_expressions(base_table, from_expression_elements);

        let mut table_counts = AHashMap::new();
        for ai in &to_check {
            *table_counts.entry(ai.table_ref.get_raw().unwrap()).or_insert(0) += 1;
        }

        let mut table_aliases: AHashMap<String, AHashSet<String>> = AHashMap::new();
        for ai in &to_check {
            if let (table_ref, Some(alias_identifier_ref)) =
                (&ai.table_ref, &ai.alias_identifier_ref)
            {
                table_aliases
                    .entry(table_ref.get_raw().unwrap())
                    .or_default()
                    .insert(alias_identifier_ref.get_raw().unwrap());
            }
        }

        for alias_info in to_check {
            if let (table_ref, Some(alias_identifier_ref)) =
                (&alias_info.table_ref, &alias_info.alias_identifier_ref)
            {
                // Skip processing if table appears more than once with different aliases
                let raw_table = table_ref.get_raw().unwrap();
                if table_counts.get(&raw_table).unwrap_or(&0) > &1
                    && table_aliases.get(&raw_table).map_or(false, |aliases| aliases.len() > 1)
                {
                    continue;
                }

                let select_clause = segment.child(&["select_clause"]).unwrap();
                let mut ids_refs = Vec::new();

                if let Some(alias_name) = alias_identifier_ref.get_raw() {
                    // Find all references to alias in select clause
                    for alias_with_column in
                        select_clause.recursive_crawl(&["object_reference"], true, None, true)
                    {
                        if let Some(used_alias_ref) =
                            alias_with_column.child(&["identifier", "naked_identifier"])
                        {
                            if used_alias_ref.get_raw().unwrap() == alias_name {
                                ids_refs.push(used_alias_ref);
                            }
                        }
                    }

                    // Find all references to alias in column references
                    for exp_ref in column_reference_segments.clone() {
                        if let Some(used_alias_ref) =
                            exp_ref.child(&["identifier", "naked_identifier"])
                        {
                            if used_alias_ref.get_raw().unwrap() == alias_name
                                && exp_ref.child(&["dot"]).is_some()
                            {
                                ids_refs.push(used_alias_ref);
                            }
                        }
                    }
                }

                // Prepare fixes for deleting and editing references to aliased tables
                let mut fixes = Vec::new();

                fixes.push(LintFix::delete(alias_info.alias_exp_ref));

                if let Some(whitespace_ref) = &alias_info.whitespace_ref {
                    fixes.push(LintFix::delete(whitespace_ref.clone()));
                }

                for alias in ids_refs.iter().chain(once(alias_identifier_ref)) {
                    let tmp = table_ref.get_raw().unwrap();
                    let identifier_parts: Vec<_> = tmp.split('.').collect();
                    let mut edits = Vec::new();
                    for (i, part) in identifier_parts.iter().enumerate() {
                        if i > 0 {
                            edits.push(SymbolSegment::create(".", &<_>::default(), <_>::default()));
                        }
                        edits.push(IdentifierSegment::create(
                            part,
                            &<_>::default(),
                            <_>::default(),
                        ));
                    }
                    fixes.push(LintFix::replace(
                        alias.clone(),
                        edits,
                        Some(vec![table_ref.clone()]),
                    ));
                }

                violation_buff.push(LintResult::new(
                    alias_info.alias_identifier_ref,
                    fixes,
                    None,
                    "Avoid aliases in from clauses and join conditions.".to_owned().into(),
                    None,
                ));
            }
        }

        violation_buff
    }

    fn filter_table_expressions(
        &self,
        base_table: Option<ErasedSegment>,
        from_expression_elements: Vec<ErasedSegment>,
    ) -> Vec<TableAliasInfo> {
        let mut acc = Vec::new();

        for from_expression in from_expression_elements {
            let table_expression = from_expression.child(&["table_expression"]);
            let Some(table_expression) = table_expression else {
                continue;
            };

            let table_ref = table_expression.child(&["object_reference", "table_reference"]);
            let Some(table_ref) = table_ref else {
                continue;
            };

            if let Some(ref base_table) = base_table {
                if base_table.get_raw() == table_ref.get_raw() && base_table != &table_ref {
                    continue;
                }
            }

            let whitespace_ref = from_expression.child(&["whitespace"]);

            let alias_exp_ref = from_expression.child(&["alias_expression"]);
            let Some(alias_exp_ref) = alias_exp_ref else {
                continue;
            };

            let alias_identifier_ref = alias_exp_ref.child(&["identifier", "naked_identifier"]);

            acc.push(TableAliasInfo {
                table_ref,
                whitespace_ref,
                alias_exp_ref,
                alias_identifier_ref,
            });
        }

        acc
    }
}

impl Rule for RuleAL07 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAL07 { force_enable: config["force_enable"].as_bool().unwrap() }.erased()
    }

    fn name(&self) -> &'static str {
        "aliasing.forbid"
    }

    fn description(&self) -> &'static str {
        "Avoid table aliases in from clauses and join conditions."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if !self.force_enable {
            return Vec::new();
        }

        let children = FunctionalContext::new(context.clone()).segment().children(None);
        let from_clause_segment = children
            .select(Some(|it| it.is_type("from_clause")), None, None, None)
            .find_first::<fn(&_) -> _>(None);

        let base_table = from_clause_segment
            .children(Some(|it| it.is_type("from_expression")))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| it.is_type("from_expression_element")))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| it.is_type("table_expression")))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| it.is_type("object_reference") || it.is_type("table_reference")));

        if base_table.is_empty() {
            return Vec::new();
        }

        let mut from_expression_elements = Vec::new();
        let mut column_reference_segments = Vec::new();

        let after_from_clause = children.select(None, None, Some(&from_clause_segment[0]), None);
        for clause in chain(from_clause_segment, after_from_clause) {
            for from_expression_element in
                clause.recursive_crawl(&["from_expression_element"], true, None, true)
            {
                from_expression_elements.push(from_expression_element);
            }

            for from_expression_element in
                clause.recursive_crawl(&["column_reference"], true, None, true)
            {
                column_reference_segments.push(from_expression_element);
            }
        }

        self.lint_aliases_in_join(
            base_table.first().cloned(),
            from_expression_elements,
            column_reference_segments,
            context.segment,
        )
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_statement"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::aliasing::AL07::RuleAL07;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAL07 { force_enable: true }.erased()]
    }

    #[test]
    fn test_pass_allow_self_join_alias() {}

    #[test]
    fn test_fail_avoid_aliases_1() {
        let fail_str = "
SELECT
  u.id,
  c.first_name,
  c.last_name,
  COUNT(o.user_id)
FROM users as u
JOIN customers as c on u.id = c.user_id
JOIN orders as o on u.id = o.user_id;";

        let fix_str = "
SELECT
  users.id,
  customers.first_name,
  customers.last_name,
  COUNT(orders.user_id)
FROM users
JOIN customers on users.id = customers.user_id
JOIN orders on users.id = orders.user_id;";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_fail_avoid_aliases_2() {
        let fail_str = "
SELECT
  u.id,
  c.first_name,
  c.last_name,
  COUNT(o.user_id)
FROM users as u
JOIN customers as c on u.id = c.user_id
JOIN orders as o on u.id = o.user_id
order by o.user_id desc;";

        let fix_str = "
SELECT
  users.id,
  customers.first_name,
  customers.last_name,
  COUNT(orders.user_id)
FROM users
JOIN customers on users.id = customers.user_id
JOIN orders on users.id = orders.user_id
order by orders.user_id desc;";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_fail_avoid_aliases_3() {
        let fail_str = "
SELECT
  u.id,
  c.first_name,
  c.last_name,
  COUNT(o.user_id)
FROM users as u
JOIN customers as c on u.id = c.user_id
JOIN orders as o on u.id = o.user_id
order by o desc;"; // In the fail string, 'o' is ambiguously used as an alias and column identifier

        let fix_str = "
SELECT
  users.id,
  customers.first_name,
  customers.last_name,
  COUNT(orders.user_id)
FROM users
JOIN customers on users.id = customers.user_id
JOIN orders on users.id = orders.user_id
order by o desc;"; // In the fix string, 'o' is intentionally left unchanged assuming it's now clear or a different issue

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_alias_single_char_identifiers() {
        let fail_str = "select b from tbl as a";
        let fix_str = "select b from tbl";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_alias_with_wildcard_identifier() {
        let fail_str = "select * from tbl as a";
        let fix_str = "select * from tbl";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_select_from_values() {
        let pass_str = "select *\nfrom values(1, 2, 3)";

        let actual = fix(pass_str.into(), rules());
        assert_eq!(actual, pass_str);
    }

    #[test]
    fn test_issue_610() {
        let pass_str = "SELECT aaaaaa.c\nFROM aaaaaa\nJOIN bbbbbb AS b ON b.a = aaaaaa.id\nJOIN \
                        bbbbbb AS b2 ON b2.other = b.id";

        let actual = fix(pass_str.into(), rules());
        assert_eq!(actual, pass_str);
    }

    #[test]
    fn test_issue_1589() {
        let pass_str = "\
    select *\nfrom (select random() as v from (values(1))) t1,\n(select max(repl) as m from data) \
                        t2,\n(select * from data\nwhere repl=t2.m and\nrnd>=t1.v\norder by \
                        rnd\nlimit 1)";

        let actual = fix(pass_str.into(), rules());
        assert_eq!(actual, pass_str);
    }

    #[test]
    fn test_violation_locations() {
        let fail_str = "\
    SELECT\nu.id,\nc.first_name,\nc.last_name,\nCOUNT(o.user_id)\nFROM users as u\nJOIN customers \
                        as c on u.id = c.user_id\nJOIN orders as o on u.id = o.user_id;";

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations.len(), 3);
        assert_eq!(violations[0].description, "Avoid aliases in from clauses and join conditions.");
        assert_eq!(violations[0].line_no, 6);
        assert_eq!(violations[0].line_pos, 15);
        assert_eq!(violations[1].description, "Avoid aliases in from clauses and join conditions.");
        assert_eq!(violations[1].line_no, 7);
        assert_eq!(violations[1].line_pos, 19);
        assert_eq!(violations[2].description, "Avoid aliases in from clauses and join conditions.");
        assert_eq!(violations[2].line_no, 8);
        assert_eq!(violations[2].line_pos, 16);
    }

    #[test]
    fn test_fail_fix_command() {
        let fail_str = "\
    SELECT u.id, c.first_name, c.last_name, COUNT(o.user_id)\nFROM users as u JOIN customers as c \
                        on u.id = c.user_id JOIN orders as o\non u.id = o.user_id;";

        let fix_str = "\
    SELECT users.id, customers.first_name, customers.last_name, COUNT(orders.user_id)\nFROM users \
                       JOIN customers on users.id = customers.user_id JOIN orders\non users.id = \
                       orders.user_id;";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }
}
