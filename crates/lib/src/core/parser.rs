#[cfg(test)]
mod tests {
    use sqruff_lib_core::parser::segments::base::Tables;

    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    #[test]
    #[ignore]
    fn test_parser_parse_error() {
        let in_str = "SELECT ;".to_string();
        let config = FluffConfig::new(<_>::default(), None, None);
        let linter = Linter::new(config, None, None, false);
        let tables = Tables::default();
        let _ = linter.parse_string(&tables, &in_str, None);
    }
}
