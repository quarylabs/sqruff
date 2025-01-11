use std::iter::zip;

use ahash::{AHashMap, AHashSet};
use nohash_hasher::{IntMap, IntSet};
use sqruff_lib_core::dialects::syntax::SyntaxSet;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, PathStep};

/// An element of the stack_positions property of DepthInfo.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StackPosition {
    pub idx: usize,
    pub len: usize,
    pub type_: Option<StackPositionType>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum StackPositionType {
    Solo,
    Start,
    End,
}

impl StackPosition {
    /// Interpret a path step for stack_positions.
    fn stack_pos_interpreter(path_step: &PathStep) -> Option<StackPositionType> {
        if path_step.code_idxs.is_empty() {
            None
        } else if path_step.code_idxs.len() == 1 {
            Some(StackPositionType::Solo)
        } else if path_step.idx == *path_step.code_idxs.first().unwrap() {
            Some(StackPositionType::Start)
        } else if path_step.idx == *path_step.code_idxs.last().unwrap() {
            Some(StackPositionType::End)
        } else {
            None
        }
    }

    /// Interpret a PathStep to construct a StackPosition
    fn from_path_step(path_step: &PathStep) -> StackPosition {
        StackPosition {
            idx: path_step.idx,
            len: path_step.len,
            type_: StackPosition::stack_pos_interpreter(path_step),
        }
    }
}

pub struct DepthMap {
    depth_info: AHashMap<u32, DepthInfo>,
}

impl DepthMap {
    fn new<'a>(raws_with_stack: impl Iterator<Item = &'a (ErasedSegment, Vec<PathStep>)>) -> Self {
        let depth_info = raws_with_stack
            .into_iter()
            .map(|(raw, stack)| (raw.id(), DepthInfo::from_stack(stack)))
            .collect();
        Self { depth_info }
    }

    pub fn get_depth_info(&self, seg: &ErasedSegment) -> DepthInfo {
        self.depth_info[&seg.id()].clone()
    }

    pub fn copy_depth_info(
        &mut self,
        anchor: &ErasedSegment,
        new_segment: &ErasedSegment,
        trim: u32,
    ) {
        self.depth_info.insert(
            new_segment.id(),
            self.get_depth_info(anchor).trim(trim.try_into().unwrap()),
        );
    }

    pub fn from_parent(parent: &ErasedSegment) -> Self {
        Self::new(parent.raw_segments_with_ancestors().iter())
    }

    pub fn from_raws_and_root(
        raw_segments: impl Iterator<Item = ErasedSegment>,
        root_segment: &ErasedSegment,
    ) -> DepthMap {
        let depth_info = raw_segments
            .into_iter()
            .map(|raw| {
                let stack = root_segment.path_to(&raw);
                (raw.id(), DepthInfo::from_stack(&stack))
            })
            .collect();

        DepthMap { depth_info }
    }
}

/// An object to hold the depth information for a specific raw segment.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DepthInfo {
    pub stack_depth: usize,
    pub stack_hashes: Vec<u64>,
    /// This is a convenience cache to speed up operations.
    pub stack_hash_set: IntSet<u64>,
    pub stack_class_types: Vec<SyntaxSet>,
    pub stack_positions: IntMap<u64, StackPosition>,
}

impl DepthInfo {
    fn from_stack(stack: &[PathStep]) -> DepthInfo {
        let stack_hashes: Vec<u64> = stack.iter().map(|ps| ps.segment.hash_value()).collect();
        let stack_hash_set: IntSet<u64> = IntSet::from_iter(stack_hashes.clone());

        let stack_class_types = stack
            .iter()
            .map(|ps| ps.segment.class_types().clone())
            .collect();

        let stack_positions: IntMap<u64, StackPosition> = zip(stack_hashes.iter(), stack.iter())
            .map(|(&hash, path)| (hash, StackPosition::from_path_step(path)))
            .collect();

        DepthInfo {
            stack_depth: stack_hashes.len(),
            stack_hashes,
            stack_hash_set,
            stack_class_types,
            stack_positions,
        }
    }

    pub fn trim(self, amount: usize) -> DepthInfo {
        // Return a DepthInfo object with some amount trimmed.
        if amount == 0 {
            // The trivial case.
            return self;
        }

        let slice_set: IntSet<_> = IntSet::from_iter(
            self.stack_hashes[self.stack_hashes.len() - amount..]
                .iter()
                .copied(),
        );

        let new_hash_set: IntSet<_> = self
            .stack_hash_set
            .difference(&slice_set)
            .copied()
            .collect();

        let stack_positions = self
            .stack_positions
            .into_iter()
            .filter(|(hash, _)| new_hash_set.contains(hash))
            .collect();

        DepthInfo {
            stack_depth: self.stack_depth - amount,
            stack_hashes: self.stack_hashes[..self.stack_hashes.len() - amount].to_vec(),
            stack_hash_set: new_hash_set,
            stack_class_types: self.stack_class_types[..self.stack_class_types.len() - amount]
                .to_vec(),
            stack_positions,
        }
    }

    pub fn common_with(&self, other: &DepthInfo) -> Vec<u64> {
        // Get the common depth and hashes with the other.
        // We use AHashSet intersection because it's efficient and hashes should be
        // unique.

        let common_hashes: AHashSet<_> = self
            .stack_hash_set
            .intersection(&other.stack_hash_set)
            .copied()
            .collect();

        // We should expect there to be _at least_ one common ancestor, because
        // they should share the same file segment. If that's not the case we
        // should error because it's likely a bug or programming error.
        assert!(
            !common_hashes.is_empty(),
            "DepthInfo comparison shares no common ancestor!"
        );

        let common_depth = common_hashes.len();
        self.stack_hashes
            .iter()
            .take(common_depth)
            .copied()
            .collect()
    }
}
