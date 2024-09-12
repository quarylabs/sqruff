/// One of `create_before`, `create_after`, `replace`, `delete` to indicate the
/// kind of fix required.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditType {
    CreateBefore,
    CreateAfter,
    Replace,
    Delete,
}
