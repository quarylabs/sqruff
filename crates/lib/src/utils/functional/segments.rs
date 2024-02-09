use crate::core::parser::segments::base::Segment;
use crate::core::templaters::base::TemplatedFile;

type PredicateType = Option<fn(&dyn Segment) -> bool>;

#[derive(Debug, Default, Clone)]
pub struct Segments {
    base: Vec<Box<dyn Segment>>,
    templated_file: Option<TemplatedFile>,
}

impl Segments {
    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn Segment>> {
        self.base.iter()
    }

    pub fn from_vec(base: Vec<Box<dyn Segment>>, templated_file: Option<TemplatedFile>) -> Self {
        Self { base, templated_file }
    }

    pub fn reversed(&self) -> Self {
        let mut base = self.base.clone();
        base.reverse();

        Self { base, templated_file: self.templated_file.clone() }
    }

    pub fn get(&self, index: usize, default: Option<Box<dyn Segment>>) -> Option<Box<dyn Segment>> {
        self.base.get(index).cloned().or(default)
    }

    pub fn first(&self) -> Option<&dyn Segment> {
        self.base.first().map(Box::as_ref)
    }

    pub fn last(&self) -> Option<&dyn Segment> {
        self.base.last().map(Box::as_ref)
    }

    #[track_caller]
    pub fn pop(&mut self) -> Box<dyn Segment> {
        self.base.pop().unwrap()
    }

    pub fn all(&self, predicate: PredicateType) -> bool {
        self.base.iter().all(|s| predicate.map_or(true, |pred| pred(s.as_ref())))
    }

    pub fn len(&self) -> usize {
        self.base.len()
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }

    pub fn new(segment: Box<dyn Segment>, templated_file: Option<TemplatedFile>) -> Self {
        Self { base: vec![segment], templated_file }
    }

    pub fn children(&self, predicate: PredicateType) -> Segments {
        let mut child_segments = Vec::new();

        for s in &self.base {
            for child in s.get_segments() {
                if let Some(ref pred) = predicate {
                    if pred(child.as_ref()) {
                        child_segments.push(child);
                    }
                } else {
                    child_segments.push(child);
                }
            }
        }

        Segments { base: child_segments, templated_file: self.templated_file.clone() }
    }

    pub fn find_last(&self, predicate: PredicateType) -> Segments {
        self.base
            .iter()
            .rev()
            .find_map(|s| {
                if predicate.as_ref().map_or(true, |p| p(s.as_ref())) {
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

    pub fn find(&self, value: &dyn Segment) -> Option<usize> {
        self.index(value)
    }

    pub fn find_first<F: Fn(&dyn Segment) -> bool>(&self, predicate: Option<F>) -> Segments {
        for s in &self.base {
            if predicate.as_ref().map_or(true, |p| p(s.as_ref())) {
                return Segments {
                    base: vec![s.clone()],
                    templated_file: self.templated_file.clone(),
                };
            }
        }

        Segments { base: vec![], templated_file: self.templated_file.clone() }
    }

    pub fn index(&self, value: &dyn Segment) -> Option<usize> {
        self.base.iter().position(|it| it.dyn_eq(value))
    }

    #[track_caller]
    pub fn select(
        &self,
        select_if: PredicateType,
        loop_while: PredicateType,
        start_seg: Option<&dyn Segment>,
        stop_seg: Option<&dyn Segment>,
    ) -> Segments {
        let start_index = start_seg
            .and_then(|seg| self.base.iter().position(|x| x.dyn_eq(seg)))
            .map_or(0, |index| index + 1);

        let stop_index = stop_seg
            .and_then(|seg| self.base.iter().position(|x| x.dyn_eq(seg)))
            .unwrap_or_else(|| self.base.len());

        let mut buff = Vec::new();

        for seg in self.base.iter().skip(start_index).take(stop_index - start_index) {
            if let Some(loop_while) = &loop_while {
                if !loop_while(seg.as_ref()) {
                    break;
                }
            }

            if select_if.as_ref().map_or(true, |f| f(seg.as_ref())) {
                buff.push(seg.clone());
            }
        }

        Segments { base: buff, templated_file: self.templated_file.clone() }
    }
}

impl<I: std::slice::SliceIndex<[Box<dyn Segment>]>> std::ops::Index<I> for Segments {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.base[index]
    }
}

impl IntoIterator for Segments {
    type Item = Box<dyn Segment>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }
}
