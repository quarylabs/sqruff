

/// Grammars are a way of composing match statements.
///
///     Any grammar must implement the `match` function. Segments can also be
///     passed to most grammars. Segments implement `match` as a classmethod. Grammars
///     implement it as an instance method.
pub trait Grammar {
    fn is_meta(&self) -> bool {
        false
    }

    /// Are we allowed to refer to keywords as strings instead of only passing grammars or segments?
    fn allow_keyboard_string_refs(&self) -> bool {
        true
    }



}