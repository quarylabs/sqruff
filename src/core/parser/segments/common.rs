#[cfg(test)]
mod test {
    use crate::core::parser::segments::base::Segment;
    use crate::core::parser::segments::test_functions::generate_test_segments_func;

    // NOTE: For legacy reasons we override this fixture for this module
    fn raw_segments() -> Vec<Box<dyn Segment>> {
        generate_test_segments_func(["bar", "foo", "bar"].to_vec())
    }
}
