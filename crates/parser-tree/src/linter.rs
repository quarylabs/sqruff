use rustc_hash::FxHashMap;

use crate::lint_fix::LintFix;
use crate::segments::AnchorEditInfo;

pub fn compute_anchor_edit_info(
    anchor_info: &mut FxHashMap<u32, AnchorEditInfo>,
    fixes: Vec<LintFix>,
) {
    for fix in fixes {
        let anchor_id = fix.anchor().id();
        anchor_info.entry(anchor_id).or_default().add(fix);
    }
}
