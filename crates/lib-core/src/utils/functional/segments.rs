use crate::dialects::syntax::SyntaxSet;
use crate::parser::segments::ErasedSegment;
use crate::templaters::TemplatedFile;

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

        for segment in &self.base {
            segments.extend(segment.recursive_crawl(types, recurse_into, &SyntaxSet::EMPTY, true));
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

    pub fn all_match<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> bool {
        self.base.iter().all(predicate)
    }

    pub fn any_match<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> bool {
        self.base.iter().any(predicate)
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

    pub fn children_all(&self) -> Segments {
        let mut child_segments = Vec::with_capacity(self.len());
        for segment in &self.base {
            child_segments.extend(segment.segments().iter().cloned());
        }
        Segments {
            base: child_segments,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn children_where<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> Segments {
        let mut child_segments = Vec::with_capacity(self.len());
        for segment in &self.base {
            for child in segment.segments() {
                if predicate(child) {
                    child_segments.push(child.clone());
                }
            }
        }
        Segments {
            base: child_segments,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn children_iter_where<F: Fn(&ErasedSegment) -> bool + 'static>(
        &self,
        predicate: F,
    ) -> impl Iterator<Item = &ErasedSegment> + '_ {
        self.base
            .iter()
            .flat_map(|segment| segment.segments().iter())
            .filter(move |child| predicate(child))
    }

    pub fn find_last_where<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> Segments {
        self.base
            .iter()
            .rev()
            .find_map(|segment| {
                if predicate(segment) {
                    Some(Segments {
                        base: vec![segment.clone()],
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

    pub fn find_first_where<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> Segments {
        for segment in &self.base {
            if predicate(segment) {
                return Segments {
                    base: vec![segment.clone()],
                    templated_file: self.templated_file.clone(),
                };
            }
        }
        Segments {
            base: vec![],
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn head(&self) -> Segments {
        if let Some(segment) = self.base.first() {
            return Segments {
                base: vec![segment.clone()],
                templated_file: self.templated_file.clone(),
            };
        }
        Segments {
            base: vec![],
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn index(&self, value: &ErasedSegment) -> Option<usize> {
        self.base.iter().position(|it| it == value)
    }

    pub fn filter<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> Segments {
        let base = self
            .base
            .iter()
            .filter(|segment| predicate(segment))
            .cloned()
            .collect();
        Segments {
            base,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn take_while<F: Fn(&ErasedSegment) -> bool>(&self, predicate: F) -> Segments {
        let base = self
            .base
            .iter()
            .take_while(|segment| predicate(segment))
            .cloned()
            .collect();
        Segments {
            base,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn iter_after<'a>(
        &'a self,
        start: &ErasedSegment,
    ) -> impl Iterator<Item = &'a ErasedSegment> + 'a {
        let start_idx = self
            .base
            .iter()
            .position(|segment| segment == start)
            .map(|index| index + 1)
            .unwrap_or(self.base.len());
        self.base[start_idx..].iter()
    }

    pub fn iter_after_while<'a, F: Fn(&ErasedSegment) -> bool + 'a>(
        &'a self,
        start: &ErasedSegment,
        while_cond: F,
    ) -> impl Iterator<Item = &'a ErasedSegment> + 'a {
        self.iter_after(start)
            .take_while(move |segment| while_cond(segment))
    }

    pub fn after(&self, start: &ErasedSegment) -> Segments {
        let start_idx = self
            .base
            .iter()
            .position(|segment| segment == start)
            .expect("start segment not found");
        let base = self.base[start_idx + 1..].to_vec();
        Segments {
            base,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn before(&self, stop: &ErasedSegment) -> Segments {
        let stop_idx = self
            .base
            .iter()
            .position(|segment| segment == stop)
            .expect("stop segment not found");
        let base = self.base[..stop_idx].to_vec();
        Segments {
            base,
            templated_file: self.templated_file.clone(),
        }
    }

    pub fn between_exclusive(&self, start: &ErasedSegment, stop: &ErasedSegment) -> Segments {
        let len = self.base.len();

        let start_idx = self
            .base
            .iter()
            .position(|s| s == start)
            .map(|i| i + 1)
            .unwrap_or(0)
            .min(len);

        let end_idx = self
            .base
            .iter()
            .position(|s| s == stop)
            .unwrap_or(len)
            .min(len);

        let base = self.base.get(start_idx..end_idx).unwrap_or(&[]).to_vec();

        Segments {
            base,
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
