pub fn object_ref_matches_table(possible_references: &[&[String]], targets: &[&[String]]) -> bool {
    // Simple case: If there are no references, assume okay.
    if possible_references.is_empty() {
        return true;
    }

    // Simple case: Reference exactly matches a target.
    for pr in possible_references {
        if targets.contains(pr) {
            return true;
        }
    }

    // Tricky case: If one is shorter than the other, check for a suffix match.
    for pr in possible_references {
        for t in targets {
            if (pr.len() < t.len() && t.ends_with(pr)) || (t.len() < pr.len() && pr.ends_with(t)) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_ref_matches_table() {
        let test_cases = vec![
            // Empty list of references is always true
            (vec![], vec![vec!["abc".to_string()]], true),
            // Simple cases: one reference, one target
            (
                vec![vec!["agent1".to_string()]],
                vec![vec!["agent1".to_string()]],
                true,
            ),
            (
                vec![vec!["agent1".to_string()]],
                vec![vec!["customer".to_string()]],
                false,
            ),
            // Multiple references. If any match, good.
            (
                vec![vec!["bar".to_string()], vec!["user_id".to_string()]],
                vec![vec!["bar".to_string()]],
                true,
            ),
            (
                vec![vec!["foo".to_string()], vec!["user_id".to_string()]],
                vec![vec!["bar".to_string()]],
                false,
            ),
            // Multiple targets. If any reference matches, good.
            (
                vec![vec!["table1".to_string()]],
                vec![
                    vec!["table1".to_string()],
                    vec!["table2".to_string()],
                    vec!["table3".to_string()],
                ],
                true,
            ),
            (
                vec![vec!["tbl2".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                false,
            ),
            (
                vec![vec!["tbl2".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl2".to_string()]],
                true,
            ),
            // Multi-part references and targets. Checks for a suffix match.
            (
                vec![vec!["rc".to_string(), "tbl1".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                false,
            ),
            (
                vec![vec!["sc".to_string(), "tbl1".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                true,
            ),
            (
                vec![vec!["cb".to_string(), "sc".to_string(), "tbl1".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                false,
            ),
            (
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                vec![vec!["db".to_string(), "sc".to_string(), "tbl1".to_string()]],
                true,
            ),
            (
                vec![vec!["public".to_string(), "agent1".to_string()]],
                vec![vec!["agent1".to_string()]],
                true,
            ),
            (
                vec![vec!["public".to_string(), "agent1".to_string()]],
                vec![vec!["public".to_string()]],
                false,
            ),
        ];

        for (possible_references, targets, expected) in test_cases {
            let pr_refs: Vec<&[String]> = possible_references.iter().map(AsRef::as_ref).collect();
            let target_refs: Vec<&[String]> = targets.iter().map(AsRef::as_ref).collect();
            assert_eq!(object_ref_matches_table(&pr_refs, &target_refs), expected);
        }
    }
}
