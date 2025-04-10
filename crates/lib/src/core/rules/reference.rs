use smol_str::SmolStr;

pub fn object_ref_matches_table(
    possible_references: &[Vec<SmolStr>],
    targets: &[Vec<SmolStr>],
) -> bool {
    // Simple case: If there are no references, assume okay.
    if possible_references.is_empty() {
        return true;
    }

    // Helper function to strip quotes from identifiers
    let strip_quotes = |s: &str| s.trim_matches('"').to_string();

    // Simple case: Reference exactly matches a target.
    for pr in possible_references {
        for t in targets {
            if pr.len() == t.len()
                && pr
                    .iter()
                    .zip(t.iter())
                    .all(|(p, t)| strip_quotes(p.as_str()) == strip_quotes(t.as_str()))
            {
                return true;
            }
        }
    }

    // Handle schema-qualified table references with aliases
    for pr in possible_references {
        for t in targets {
            // If the reference is just the alias (e.g. "user_profiles")
            if pr.len() == 1
                && t.len() == 1
                && strip_quotes(pr[0].as_str()) == strip_quotes(t[0].as_str())
            {
                return true;
            }
            // If the reference includes schema (e.g. ["public", "user_profiles"])
            if pr.len() == 2
                && t.len() == 1
                && strip_quotes(pr[1].as_str()) == strip_quotes(t[0].as_str())
            {
                return true;
            }
        }
    }

    // Tricky case: If one is shorter than the other, check for a suffix match.
    for pr in possible_references {
        for t in targets {
            match pr.len().cmp(&t.len()) {
                std::cmp::Ordering::Less => {
                    let suffix_match = pr
                        .iter()
                        .zip(t[t.len() - pr.len()..].iter())
                        .all(|(p, t)| strip_quotes(p.as_str()) == strip_quotes(t.as_str()));
                    if suffix_match {
                        return true;
                    }
                }
                std::cmp::Ordering::Greater => {
                    let suffix_match = t
                        .iter()
                        .zip(pr[pr.len() - t.len()..].iter())
                        .all(|(t, p)| strip_quotes(t.as_str()) == strip_quotes(p.as_str()));
                    if suffix_match {
                        return true;
                    }
                }
                std::cmp::Ordering::Equal => {}
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
