use std::collections::BTreeSet;
use std::ops::Deref;

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTraitImpl, MatchableTrait};
use sqruff_lib_dialects::kind_to_dialect;
use strum::IntoEnumIterator;

/// Recursively walk a Matchable grammar tree and collect all `Ref` reference names.
fn collect_refs(matchable: &Matchable, dialect: &Dialect, refs: &mut BTreeSet<String>) {
    match matchable.deref() {
        MatchableTraitImpl::Ref(r) => {
            refs.insert(r.reference.to_string());
            if let Some(exclude) = &r.exclude {
                collect_refs(exclude, dialect, refs);
            }
            for elem in &r.terminators {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::NodeMatcher(nm) => {
            // Trigger lazy grammar initialization and walk it.
            let grammar = nm.match_grammar(dialect);
            collect_refs(&grammar, dialect, refs);
        }
        MatchableTraitImpl::Sequence(seq) => {
            for elem in &seq.terminators {
                collect_refs(elem, dialect, refs);
            }
            for elem in seq.elements() {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::Bracketed(br) => {
            for elem in &br.this.terminators {
                collect_refs(elem, dialect, refs);
            }
            for elem in br.this.elements() {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::AnyNumberOf(any) => {
            if let Some(exclude) = &any.exclude {
                collect_refs(exclude, dialect, refs);
            }
            for elem in &any.terminators {
                collect_refs(elem, dialect, refs);
            }
            for elem in any.elements() {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::Delimited(del) => {
            collect_refs(&del.delimiter, dialect, refs);
            if let Some(exclude) = &del.base.exclude {
                collect_refs(exclude, dialect, refs);
            }
            for elem in &del.base.terminators {
                collect_refs(elem, dialect, refs);
            }
            for elem in del.base.elements() {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::Anything(any) => {
            for elem in &any.terminators {
                collect_refs(elem, dialect, refs);
            }
        }
        MatchableTraitImpl::Conditional(_) => {}
        // Leaf nodes with no sub-matchable references:
        MatchableTraitImpl::StringParser(_)
        | MatchableTraitImpl::TypedParser(_)
        | MatchableTraitImpl::CodeParser(_)
        | MatchableTraitImpl::MultiStringParser(_)
        | MatchableTraitImpl::RegexParser(_)
        | MatchableTraitImpl::MetaSegment(_)
        | MatchableTraitImpl::NonCodeMatcher(_)
        | MatchableTraitImpl::Nothing(_)
        | MatchableTraitImpl::BracketedSegmentMatcher(_)
        | MatchableTraitImpl::LookaheadExclude(_) => {}
    }
}

/// Collect all bracket pair segment references from a dialect.
fn collect_bracket_refs(dialect: &Dialect) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for set_name in ["bracket_pairs", "angle_bracket_pairs"] {
        for (_bracket_type, start_ref, end_ref, _persists) in dialect.bracket_sets(set_name) {
            refs.insert(start_ref.to_string());
            refs.insert(end_ref.to_string());
        }
    }
    refs
}

#[test]
fn all_dialect_references_resolve() {
    let mut failures = Vec::new();

    for kind in DialectKind::iter() {
        let Some(dialect) = kind_to_dialect(&kind, None) else {
            continue;
        };

        let library_names: BTreeSet<String> = dialect.library_names().map(String::from).collect();

        // Collect all Ref references by walking every library entry.
        // Use a visited set to avoid infinite recursion through NodeMatcher cycles.
        let mut all_refs = BTreeSet::new();
        let mut visited = BTreeSet::new();

        fn walk_entry(
            name: &str,
            dialect: &Dialect,
            library_names: &BTreeSet<String>,
            all_refs: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
        ) {
            if !visited.insert(name.to_string()) {
                return;
            }
            if !library_names.contains(name) {
                return;
            }
            let matchable = dialect.r#ref(name);
            let mut entry_refs = BTreeSet::new();
            collect_refs(&matchable, dialect, &mut entry_refs);
            for ref_name in &entry_refs {
                walk_entry(ref_name, dialect, library_names, all_refs, visited);
            }
            all_refs.extend(entry_refs);
        }

        for name in &library_names {
            walk_entry(name, &dialect, &library_names, &mut all_refs, &mut visited);
        }

        // Also collect bracket pair references.
        let bracket_refs = collect_bracket_refs(&dialect);
        all_refs.extend(bracket_refs);

        // Check every referenced name exists in the library.
        for ref_name in &all_refs {
            if !library_names.contains(ref_name) {
                failures.push(format!(
                    "Dialect {}: Ref '{}' not found in library",
                    kind.name(),
                    ref_name,
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Found {} unresolved references:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
