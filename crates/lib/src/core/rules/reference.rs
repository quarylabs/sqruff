use smol_str::SmolStr;

pub fn object_ref_matches_table(
    possible_references: &[Vec<SmolStr>],
    targets: &[Vec<SmolStr>],
) -> bool {
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
            if (pr.len() < t.len() && pr == &t[t.len() - pr.len()..])
                || (t.len() < pr.len() && t == &pr[pr.len() - t.len()..])
            {
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
            (vec![], vec![vec!["abc".into()]], true),
            // Simple cases: one reference, one target
            (
                vec![vec!["agent1".into()]],
                vec![vec!["agent1".into()]],
                true,
            ),
            (
                vec![vec!["agent1".into()]],
                vec![vec!["customer".into()]],
                false,
            ),
            // Multiple references. If any match, good.
            (
                vec![vec!["bar".into()], vec!["user_id".into()]],
                vec![vec!["bar".into()]],
                true,
            ),
            (
                vec![vec!["foo".into()], vec!["user_id".into()]],
                vec![vec!["bar".into()]],
                false,
            ),
            // Multiple targets. If any reference matches, good.
            (
                vec![vec!["table1".into()]],
                vec![
                    vec!["table1".into()],
                    vec!["table2".into()],
                    vec!["table3".into()],
                ],
                true,
            ),
            (
                vec![vec!["tbl2".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                false,
            ),
            (
                vec![vec!["tbl2".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl2".into()]],
                true,
            ),
            // Multipart references and targets. Checks for a suffix match.
            (
                vec![vec!["Arc".into(), "tbl1".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                false,
            ),
            (
                vec![vec!["sc".into(), "tbl1".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                true,
            ),
            (
                vec![vec!["cb".into(), "sc".into(), "tbl1".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                false,
            ),
            (
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                vec![vec!["db".into(), "sc".into(), "tbl1".into()]],
                true,
            ),
            (
                vec![vec!["public".into(), "agent1".into()]],
                vec![vec!["agent1".into()]],
                true,
            ),
            (
                vec![vec!["public".into(), "agent1".into()]],
                vec![vec!["public".into()]],
                false,
            ),
        ];

        for (possible_references, targets, expected) in test_cases {
            assert_eq!(
                object_ref_matches_table(&possible_references, &targets),
                expected
            );
        }
    }
}
