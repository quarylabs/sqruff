use std::ops::Range;

use smol_str::SmolStr;

/// A stored reference to a fix in the non-templated file.
#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub struct SourceFix {
    pub(crate) edit: SmolStr,
    pub(crate) source_slice: Range<usize>,
    // TODO: It might be possible to refactor this to not require
    // a templated_slice (because in theory it's unnecessary).
    // However much of the fix handling code assumes we need
    // a position in the templated file to interpret it.
    // More work required to achieve that if desired.
    pub(crate) templated_slice: Range<usize>,
}

impl SourceFix {
    pub fn new(edit: SmolStr, source_slice: Range<usize>, templated_slice: Range<usize>) -> Self {
        SourceFix {
            edit,
            source_slice,
            templated_slice,
        }
    }
}

/// An edit patch for a source file.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FixPatch {
    templated_slice: Range<usize>,
    pub fixed_raw: SmolStr,
    // The patch category, functions mostly for debugging and explanation
    // than for function. It allows traceability of *why* this patch was
    // generated. It has no significance for processing. Brought over from sqlfluff
    // patch_category: FixPatchCategory,
    pub source_slice: Range<usize>,
    templated_str: String,
    source_str: String,
}

impl FixPatch {
    pub fn new(
        templated_slice: Range<usize>,
        fixed_raw: SmolStr,
        source_slice: Range<usize>,
        templated_str: String,
        source_str: String,
    ) -> Self {
        FixPatch {
            templated_slice,
            fixed_raw,
            source_slice,
            templated_str,
            source_str,
        }
    }

    /// Generate a tuple of this fix for deduping.
    pub fn dedupe_tuple(&self) -> Range<usize> {
        self.source_slice.clone()
    }
}
