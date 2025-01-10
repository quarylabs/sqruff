use std::ops::Range;

use crate::dialects::syntax::SyntaxSet;
use crate::parser::segments::base::ErasedSegment;
use crate::templaters::base::TemplatedFile;

type PredicateType = Option<fn(&ErasedSegment) -> bool>;

#[derive(Debug, Default, Clone)]
pub struct Segments {
    pub base: Vec<ErasedSegment>,
    templated_file: Option<TemplatedFile>,
}

impl Segments {
    pub fn into_vec(self) -> Vec<ErasedSegment> {
        self.base
    }

    pub fn iter(&self) -> impl Iterator<Item = &ErasedSegment> {
        self.base.iter()
    }

    pub fn recursive_crawl(&self, types: &SyntaxSet, recurse_into: bool) -> Segments {
        let mut segments = Vec::new();

        for s in &self.base {
            segments.extend(s.recursive_crawl(types, recurse_into, &SyntaxSet::EMPTY, true));
        }

        Segments::from_vec(segments, self.templated_file.clone())
    }

    pub fn iterate_segments(&self) -> impl Iterator<Item = Segments> + '_ {
        let mut iter = self.base.iter();

        std::iter::from_fn(move || {
            let segment = iter.next()?;
            Segments::new(segment.clone(), self.templated_file.clone()).into()
        })
    }

    pub fn from_vec(base: Vec<ErasedSegment>, templated_file: Option<TemplatedFile>) -> Self {
        Self {
            base,
            templated_file,
        }
    }

    pub fn reversed(&self) -> Self {
        let mut base = self.base.clone();
        base.reverse();

        Self {
            base,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn get(&self, index: usize, default: Option<ErasedSegment>) -> Option<ErasedSegment> {
        self.base.get(index).cloned().or(default)
    }

    pub fn first(&self) -> Option<&ErasedSegment> {
        self.base.first()
    }

    pub fn last(&self) -> Option<&ErasedSegment> {
        self.base.last()
    }

    #[track_caller]
    pub fn pop(&mut self) -> ErasedSegment {
        self.base.pop().unwrap()
    }

    pub fn all(&self, predicate: PredicateType) -> bool {
        self.base
            .iter()
            .all(|s| predicate.is_none_or(|pred| pred(s)))
    }

    pub fn any(&self, predicate: PredicateType) -> bool {
        self.base
            .iter()
            .any(|s| predicate.is_none_or(|pred| pred(s)))
    }

    pub fn len(&self) -> usize {
        self.base.len()
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }

    pub fn new(segment: ErasedSegment, templated_file: Option<TemplatedFile>) -> Self {
        Self {
            base: vec![segment],
            templated_file,
        }
    }

    pub fn children(&self, predicate: PredicateType) -> Segments {
        let mut child_segments = Vec::with_capacity(predicate.map_or(0, |_| self.len()));

        for s in &self.base {
            for child in s.segments() {
                if let Some(ref pred) = predicate {
                    if pred(child) {
                        child_segments.push(child.clone());
                    }
                } else {
                    child_segments.push(child.clone());
                }
            }
        }

        Segments {
            base: child_segments,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn find_last(&self, predicate: PredicateType) -> Segments {
        self.base
            .iter()
            .rev()
            .find_map(|s| {
                if predicate.as_ref().is_none_or(|p| p(s)) {
                    Some(Segments {
                        base: vec![s.clone()],
                        templated_file: self.templated_file.clone(),
                    })
                } else {
                    None
                }
            })
            .unwrap_or_else(|| Segments {
                base: vec![],
                templated_file: self.templated_file.clone(),
            })
    }

    pub fn find(&self, value: &ErasedSegment) -> Option<usize> {
        self.index(value)
    }

    pub fn find_first<F: Fn(&ErasedSegment) -> bool>(&self, predicate: Option<F>) -> Segments {
        for s in &self.base {
            if predicate.as_ref().is_none_or(|p| p(s)) {
                return Segments {
                    base: vec![s.clone()],
                    templated_file: self.templated_file.clone(),
                };
            }
        }

        Segments {
            base: vec![],
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn index(&self, value: &ErasedSegment) -> Option<usize> {
        self.base.iter().position(|it| it == value)
    }

    #[track_caller]
    pub fn select<SelectIf: Fn(&ErasedSegment) -> bool>(
        &self,
        select_if: Option<SelectIf>,
        loop_while: PredicateType,
        start_seg: Option<&ErasedSegment>,
        stop_seg: Option<&ErasedSegment>,
    ) -> Segments {
        let start_index = start_seg
            .map(|seg| self.base.iter().position(|x| x == seg).unwrap() as isize)
            .unwrap_or(-1);

        let stop_index = stop_seg
            .map(|seg| self.base.iter().position(|x| x == seg).unwrap() as isize)
            .unwrap_or_else(|| self.base.len() as isize);

        let mut buff = Vec::new();

        for seg in pyslice(&self.base, start_index + 1..stop_index) {
            if let Some(loop_while) = &loop_while {
                if !loop_while(seg) {
                    break;
                }
            }

            if select_if.as_ref().is_none_or(|f| f(seg)) {
                buff.push(seg.clone());
            }
        }

        Segments {
            base: buff,
            templated_file: self.templated_file.clone(),
        }
    }
}

impl<I: std::slice::SliceIndex<[ErasedSegment]>> std::ops::Index<I> for Segments {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.base[index]
    }
}

impl IntoIterator for Segments {
    type Item = ErasedSegment;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }
}

fn pyslice<T>(collection: &[T], Range { start, end }: Range<isize>) -> impl Iterator<Item = &T> {
    let slice = slyce::Slice {
        start: start.into(),
        end: end.into(),
        step: None,
    };
    slice.apply(collection)
}
