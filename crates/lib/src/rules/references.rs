use hashbrown::HashSet;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::rules::ErasedRule;

pub mod rf01;
pub mod rf02;
pub mod rf03;
pub mod rf04;
pub mod rf05;
pub mod rf06;
pub mod rf07;

pub fn rules() -> Vec<ErasedRule> {
    use crate::core::rules::Erased as _;

    vec![
        rf01::RuleRF01::default().erased(),
        rf02::RuleRF02::default().erased(),
        rf03::RuleRF03::default().erased(),
        rf04::RuleRF04::default().erased(),
        rf05::RuleRF05::default().erased(),
        rf06::RuleRF06::default().erased(),
        rf07::RuleRF07::default().erased(),
    ]
}

fn postgres_order_by_alias_references(
    select: &ErasedSegment,
    dialect: DialectKind,
) -> HashSet<u32> {
    if dialect != DialectKind::Postgres {
        return HashSet::new();
    }
    let Some(order_by) = select.child(const { &SyntaxSet::single(SyntaxKind::OrderbyClause) })
    else {
        return HashSet::new();
    };
    let Some(select_clause) = select.child(const { &SyntaxSet::single(SyntaxKind::SelectClause) })
    else {
        return HashSet::new();
    };

    let aliases: HashSet<_> = select_clause
        .children(const { &SyntaxSet::single(SyntaxKind::SelectClauseElement) })
        .filter_map(|target| {
            target
                .child(const { &SyntaxSet::single(SyntaxKind::AliasExpression) })?
                .child(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::Identifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                )
        })
        .map(|identifier| postgres_identifier_key(&identifier))
        .collect();
    if aliases.is_empty() {
        return HashSet::new();
    }

    // PostgreSQL gives output aliases priority for a bare sort name. Column
    // references inside a sort expression still refer to the input tables.
    order_by
        .children(const { &SyntaxSet::new(&[SyntaxKind::ColumnReference, SyntaxKind::Expression]) })
        .filter_map(|sort_key| {
            let (reference, identifier) = bare_sort_reference(sort_key)?;
            aliases
                .contains(&postgres_identifier_key(identifier))
                .then(|| reference.id())
        })
        .collect()
}

fn bare_sort_reference(mut segment: &ErasedSegment) -> Option<(&ErasedSegment, &ErasedSegment)> {
    while matches!(
        segment.get_type(),
        SyntaxKind::Expression | SyntaxKind::Bracketed
    ) {
        let mut contents = segment.segments().iter().filter(|child| {
            child.is_code()
                && !matches!(
                    child.get_type(),
                    SyntaxKind::StartBracket | SyntaxKind::EndBracket
                )
        });
        segment = contents.next()?;
        if contents.next().is_some() {
            return None;
        }
    }

    let [identifier] = segment.segments() else {
        return None;
    };
    (segment.is_type(SyntaxKind::ColumnReference)
        && matches!(
            identifier.get_type(),
            SyntaxKind::NakedIdentifier | SyntaxKind::Identifier | SyntaxKind::QuotedIdentifier
        ))
    .then_some((segment, identifier))
}

fn postgres_identifier_key(identifier: &ErasedSegment) -> String {
    let raw = identifier.raw().as_str();
    if identifier.is_type(SyntaxKind::QuotedIdentifier) {
        // Keep PostgreSQL's doubled quotes and literal backslashes distinct.
        return raw
            .strip_prefix('"')
            .and_then(|name| name.strip_suffix('"'))
            .unwrap_or(raw)
            .to_owned();
    }
    raw.to_ascii_lowercase()
}
