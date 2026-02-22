/// Canonical function casing for ClickHouse case-sensitive built-ins.
///
/// Keep this list sorted by lowercase key and update via:
/// `.hacking/scripts/add_clickhouse_function_casing.py`.
pub(crate) const CLICKHOUSE_FUNCTION_CASING: &[(&str, &str)] = &[
    // BEGIN CLICKHOUSE_FUNCTION_CASING_MAP
    ("tointervalday", "toIntervalDay"),
    ("tointervalhour", "toIntervalHour"),
    ("tointervalmicrosecond", "toIntervalMicrosecond"),
    ("tointervalmillisecond", "toIntervalMillisecond"),
    ("tointervalminute", "toIntervalMinute"),
    ("tointervalmonth", "toIntervalMonth"),
    ("tointervalnanosecond", "toIntervalNanosecond"),
    ("tointervalquarter", "toIntervalQuarter"),
    ("tointervalsecond", "toIntervalSecond"),
    ("tointervalweek", "toIntervalWeek"),
    ("tointervalyear", "toIntervalYear"),
    ("toyyyymmdd", "toYYYYMMDD"),
    // END CLICKHOUSE_FUNCTION_CASING_MAP
];

pub(crate) fn canonical_clickhouse_function_name(function_name: &str) -> Option<&'static str> {
    let lookup_key = function_name.to_ascii_lowercase();
    CLICKHOUSE_FUNCTION_CASING
        .binary_search_by_key(&lookup_key.as_str(), |(lowercase_name, _)| *lowercase_name)
        .ok()
        .map(|index| CLICKHOUSE_FUNCTION_CASING[index].1)
}

#[cfg(test)]
mod tests {
    use super::CLICKHOUSE_FUNCTION_CASING;
    use super::canonical_clickhouse_function_name;

    #[test]
    fn clickhouse_function_casing_lookup_works() {
        assert_eq!(
            canonical_clickhouse_function_name("tointervalminute"),
            Some("toIntervalMinute")
        );
        assert_eq!(
            canonical_clickhouse_function_name("TOINTERVALMONTH"),
            Some("toIntervalMonth")
        );
        assert_eq!(canonical_clickhouse_function_name("sum"), None);
    }

    #[test]
    fn clickhouse_function_casing_entries_are_sorted() {
        for pair in CLICKHOUSE_FUNCTION_CASING.windows(2) {
            assert!(pair[0].0 <= pair[1].0, "entries must be sorted");
        }
    }
}
