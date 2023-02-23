#[derive(Debug, Clone)]
pub struct BaseSegment {}

/// An element of the response to BaseSegment.path_to().
///     Attributes:
///         segment (:obj:`BaseSegment`): The segment in the chain.
///         idx (int): The index of the target within its `segment`.
///         len (int): The number of children `segment` has.
pub struct PathStep {
    segment: BaseSegment,
    idx: usize,
    len: usize,
}
