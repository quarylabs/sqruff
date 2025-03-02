use itertools::{Itertools, enumerate};
use smol_str::{SmolStr, ToSmolStr};

use crate::dialects::init::DialectKind;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::base::ErasedSegment;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ObjectReferenceLevel {
    Object = 1,
    Table = 2,
    Schema = 3,
}

#[derive(Clone, Debug)]
pub struct ObjectReferencePart {
    pub part: String,
    pub segments: Vec<ErasedSegment>,
}

#[derive(Clone)]
pub struct ObjectReferenceSegment(pub ErasedSegment, pub ObjectReferenceKind);

#[derive(Clone)]
pub enum ObjectReferenceKind {
    Object,
    Table,
    WildcardIdentifier,
}

impl ObjectReferenceSegment {
    pub fn is_qualified(&self) -> bool {
        self.iter_raw_references().len() > 1
    }

    pub fn qualification(&self) -> &'static str {
        if self.is_qualified() {
            "qualified"
        } else {
            "unqualified"
        }
    }

    pub fn extract_possible_references(
        &self,
        level: ObjectReferenceLevel,
        dialect: DialectKind,
    ) -> Vec<ObjectReferencePart> {
        let refs = self.iter_raw_references();

        match dialect {
            DialectKind::Bigquery => {
                if level == ObjectReferenceLevel::Schema && refs.len() >= 3 {
                    return vec![refs[0].clone()];
                }

                if level == ObjectReferenceLevel::Table {
                    return refs.into_iter().take(3).collect_vec();
                }

                if level == ObjectReferenceLevel::Object && refs.len() >= 3 {
                    return vec![refs[1].clone(), refs[2].clone()];
                }

                self.extract_possible_references(level, DialectKind::Ansi)
            }
            _ => {
                let level = level as usize;
                if refs.len() >= level && level > 0 {
                    refs.get(refs.len() - level).cloned().into_iter().collect()
                } else {
                    vec![]
                }
            }
        }
    }

    pub fn extract_possible_multipart_references(
        &self,
        levels: &[ObjectReferenceLevel],
    ) -> Vec<Vec<ObjectReferencePart>> {
        self.extract_possible_multipart_references_inner(levels, self.0.dialect())
    }

    pub fn extract_possible_multipart_references_inner(
        &self,
        levels: &[ObjectReferenceLevel],
        dialect_kind: DialectKind,
    ) -> Vec<Vec<ObjectReferencePart>> {
        match dialect_kind {
            DialectKind::Bigquery => {
                let levels_tmp: Vec<_> = levels.iter().map(|level| *level as usize).collect();
                let min_level: usize = *levels_tmp.iter().min().unwrap();
                let max_level: usize = *levels_tmp.iter().max().unwrap();
                let refs = self.iter_raw_references();

                if max_level == ObjectReferenceLevel::Schema as usize && refs.len() >= 3 {
                    return vec![refs[0..=max_level - min_level].to_vec()];
                }

                self.extract_possible_multipart_references_inner(levels, DialectKind::Ansi)
            }
            _ => {
                let refs = self.iter_raw_references();
                let mut sorted_levels = levels.to_vec();
                sorted_levels.sort_unstable();

                if let (Some(&min_level), Some(&max_level)) =
                    (sorted_levels.first(), sorted_levels.last())
                {
                    if refs.len() >= max_level as usize {
                        let start = refs.len() - max_level as usize;
                        let end = refs.len() - min_level as usize + 1;
                        if start < end {
                            return vec![refs[start..end].to_vec()];
                        }
                    }
                }
                vec![]
            }
        }
    }

    pub fn iter_raw_references(&self) -> Vec<ObjectReferencePart> {
        match self.1 {
            ObjectReferenceKind::Table if self.0.dialect() == DialectKind::Bigquery => {
                let mut acc = Vec::new();
                let mut parts = Vec::new();
                let mut elems_for_parts = Vec::new();

                let mut flush =
                    |parts: &mut Vec<SmolStr>, elems_for_parts: &mut Vec<ErasedSegment>| {
                        acc.push(ObjectReferencePart {
                            part: std::mem::take(parts).iter().join(""),
                            segments: std::mem::take(elems_for_parts),
                        });
                    };

                for elem in self.0.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                            SyntaxKind::Literal,
                            SyntaxKind::Dash,
                            SyntaxKind::Dot,
                            SyntaxKind::Star,
                        ])
                    },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                ) {
                    if !elem.is_type(SyntaxKind::Dot) {
                        if elem.is_type(SyntaxKind::Identifier)
                            || elem.is_type(SyntaxKind::NakedIdentifier)
                            || elem.is_type(SyntaxKind::QuotedIdentifier)
                        {
                            let raw = elem.raw();
                            let elem_raw = raw.trim_matches('`');
                            let elem_subparts = elem_raw.split(".").collect_vec();
                            let elem_subparts_count = elem_subparts.len();

                            for (idx, part) in enumerate(elem_subparts) {
                                parts.push(part.to_smolstr());
                                elems_for_parts.push(elem.clone());

                                if idx != elem_subparts_count - 1 {
                                    flush(&mut parts, &mut elems_for_parts);
                                }
                            }
                        } else {
                            parts.push(elem.raw().to_smolstr());
                            elems_for_parts.push(elem);
                        }
                    } else {
                        flush(&mut parts, &mut elems_for_parts);
                    }
                }

                if !parts.is_empty() {
                    flush(&mut parts, &mut elems_for_parts);
                }

                acc
            }
            ObjectReferenceKind::Object | ObjectReferenceKind::Table => {
                let mut acc = Vec::new();

                for elem in self.0.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                ) {
                    acc.extend(self.iter_reference_parts(elem));
                }

                acc
            }
            ObjectReferenceKind::WildcardIdentifier => {
                let mut acc = Vec::new();

                for elem in self.0.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::Star,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                ) {
                    acc.extend(self.iter_reference_parts(elem));
                }

                acc
            }
        }
    }

    fn iter_reference_parts(&self, elem: ErasedSegment) -> Vec<ObjectReferencePart> {
        let mut acc = Vec::new();

        let raw = elem.raw();
        let parts = raw.split('.');

        for part in parts {
            acc.push(ObjectReferencePart {
                part: part.into(),
                segments: vec![elem.clone()],
            });
        }

        acc
    }
}
