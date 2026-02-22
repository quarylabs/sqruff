/// Canonical function casing for ClickHouse case-sensitive built-ins.
///
/// Keep this list sorted by lowercase key and update via:
/// `.hacking/scripts/add_clickhouse_function_casing.py`.
pub(crate) const CLICKHOUSE_FUNCTION_CASING: &[(&str, &str)] = &[
    // BEGIN CLICKHOUSE_FUNCTION_CASING_MAP
    ("__actionname", "__actionName"),
    ("__bitboolmaskand", "__bitBoolMaskAnd"),
    ("__bitboolmaskor", "__bitBoolMaskOr"),
    ("__bitswaplasttwo", "__bitSwapLastTwo"),
    ("__bitwrapperfunc", "__bitWrapperFunc"),
    ("__getscalar", "__getScalar"),
    ("__scalarsubqueryresult", "__scalarSubqueryResult"),
    ("accuratecast", "accurateCast"),
    ("accuratecastordefault", "accurateCastOrDefault"),
    ("accuratecastornull", "accurateCastOrNull"),
    ("acosh", "acosh"),
    ("adddays", "addDays"),
    ("addhours", "addHours"),
    ("addinterval", "addInterval"),
    ("addmicroseconds", "addMicroseconds"),
    ("addmilliseconds", "addMilliseconds"),
    ("addminutes", "addMinutes"),
    ("addmonths", "addMonths"),
    ("addnanoseconds", "addNanoseconds"),
    ("addquarters", "addQuarters"),
    ("addresstoline", "addressToLine"),
    ("addresstolinewithinlines", "addressToLineWithInlines"),
    ("addresstosymbol", "addressToSymbol"),
    ("addseconds", "addSeconds"),
    ("addtupleofintervals", "addTupleOfIntervals"),
    ("addweeks", "addWeeks"),
    ("addyears", "addYears"),
    ("aes_decrypt_mysql", "aes_decrypt_mysql"),
    ("aes_encrypt_mysql", "aes_encrypt_mysql"),
    ("aggthrow", "aggThrow"),
    ("alphatokens", "alphaTokens"),
    ("and", "and"),
    ("any", "any"),
    ("any_respect_nulls", "any_respect_nulls"),
    ("anyheavy", "anyHeavy"),
    ("anylast", "anyLast"),
    ("anylast_respect_nulls", "anyLast_respect_nulls"),
    ("appendtrailingcharifabsent", "appendTrailingCharIfAbsent"),
    ("argmax", "argMax"),
    ("argmin", "argMin"),
    ("array", "array"),
    ("arrayall", "arrayAll"),
    ("arrayauc", "arrayAUC"),
    ("arrayaucpr", "arrayAUCPR"),
    ("arrayavg", "arrayAvg"),
    ("arraycompact", "arrayCompact"),
    ("arrayconcat", "arrayConcat"),
    ("arraycount", "arrayCount"),
    ("arraycumsum", "arrayCumSum"),
    ("arraycumsumnonnegative", "arrayCumSumNonNegative"),
    ("arraydifference", "arrayDifference"),
    ("arraydistinct", "arrayDistinct"),
    ("arraydotproduct", "arrayDotProduct"),
    ("arrayelement", "arrayElement"),
    ("arrayelementornull", "arrayElementOrNull"),
    ("arrayenumerate", "arrayEnumerate"),
    ("arrayenumeratedense", "arrayEnumerateDense"),
    ("arrayenumeratedenseranked", "arrayEnumerateDenseRanked"),
    ("arrayenumerateuniq", "arrayEnumerateUniq"),
    ("arrayenumerateuniqranked", "arrayEnumerateUniqRanked"),
    ("arrayexists", "arrayExists"),
    ("arrayfill", "arrayFill"),
    ("arrayfilter", "arrayFilter"),
    ("arrayfirst", "arrayFirst"),
    ("arrayfirstindex", "arrayFirstIndex"),
    ("arrayfirstornull", "arrayFirstOrNull"),
    ("arrayflatten", "arrayFlatten"),
    ("arrayfold", "arrayFold"),
    ("arrayintersect", "arrayIntersect"),
    ("arrayjaccardindex", "arrayJaccardIndex"),
    ("arrayjoin", "arrayJoin"),
    ("arraylast", "arrayLast"),
    ("arraylastindex", "arrayLastIndex"),
    ("arraylastornull", "arrayLastOrNull"),
    ("arraylevenshteindistance", "arrayLevenshteinDistance"),
    (
        "arraylevenshteindistanceweighted",
        "arrayLevenshteinDistanceWeighted",
    ),
    ("arraymap", "arrayMap"),
    ("arraymax", "arrayMax"),
    ("arraymin", "arrayMin"),
    ("arraynormalizedgini", "arrayNormalizedGini"),
    ("arraypartialreversesort", "arrayPartialReverseSort"),
    ("arraypartialsort", "arrayPartialSort"),
    ("arraypopback", "arrayPopBack"),
    ("arraypopfront", "arrayPopFront"),
    ("arrayproduct", "arrayProduct"),
    ("arraypushback", "arrayPushBack"),
    ("arraypushfront", "arrayPushFront"),
    ("arrayrandomsample", "arrayRandomSample"),
    ("arrayreduce", "arrayReduce"),
    ("arrayreduceinranges", "arrayReduceInRanges"),
    ("arrayresize", "arrayResize"),
    ("arrayreverse", "arrayReverse"),
    ("arrayreversefill", "arrayReverseFill"),
    ("arrayreversesort", "arrayReverseSort"),
    ("arrayreversesplit", "arrayReverseSplit"),
    ("arrayrocauc", "arrayROCAUC"),
    ("arrayrotateleft", "arrayRotateLeft"),
    ("arrayrotateright", "arrayRotateRight"),
    ("arrayshiftleft", "arrayShiftLeft"),
    ("arrayshiftright", "arrayShiftRight"),
    ("arrayshingles", "arrayShingles"),
    ("arraysimilarity", "arraySimilarity"),
    ("arrayslice", "arraySlice"),
    ("arraysort", "arraySort"),
    ("arraysplit", "arraySplit"),
    ("arraystringconcat", "arrayStringConcat"),
    ("arraysum", "arraySum"),
    ("arraysymmetricdifference", "arraySymmetricDifference"),
    ("arrayunion", "arrayUnion"),
    ("arrayuniq", "arrayUniq"),
    ("arraywithconstant", "arrayWithConstant"),
    ("arrayzip", "arrayZip"),
    ("arrayzipunaligned", "arrayZipUnaligned"),
    ("asinh", "asinh"),
    ("assumenotnull", "assumeNotNull"),
    ("atanh", "atanh"),
    ("avgweighted", "avgWeighted"),
    ("bar", "bar"),
    ("base58decode", "base58Decode"),
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

/// ClickHouse function names known to be case-insensitive.
///
/// For these functions, CP03 should follow user capitalisation policy instead of
/// forcing canonical case.
pub(crate) const CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS: &[&str] = &[
    // BEGIN CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS
    "_cast",
    "abs",
    "acos",
    "adddate",
    "age",
    "analysisofvariance",
    "anova",
    "any_value",
    "any_value_respect_nulls",
    "approx_top_count",
    "approx_top_k",
    "approx_top_sum",
    "arraypartialshuffle",
    "arrayshuffle",
    "ascii",
    "asin",
    "atan",
    "atan2",
    "avg",
    "first_value",
    "first_value_respect_nulls",
    "flatten",
    "last_value",
    "last_value_respect_nulls",
    // END CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS
];

pub(crate) fn canonical_clickhouse_function_name(function_name: &str) -> Option<&'static str> {
    let lookup_key = function_name.to_ascii_lowercase();
    CLICKHOUSE_FUNCTION_CASING
        .binary_search_by_key(&lookup_key.as_str(), |(lowercase_name, _)| *lowercase_name)
        .ok()
        .map(|index| CLICKHOUSE_FUNCTION_CASING[index].1)
}

pub(crate) fn is_clickhouse_case_insensitive_function(function_name: &str) -> bool {
    let lookup_key = function_name.to_ascii_lowercase();
    CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS
        .binary_search(&lookup_key.as_str())
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS;
    use super::CLICKHOUSE_FUNCTION_CASING;
    use super::canonical_clickhouse_function_name;
    use super::is_clickhouse_case_insensitive_function;

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
        assert_eq!(
            canonical_clickhouse_function_name("arrayauc"),
            Some("arrayAUC")
        );
        assert_eq!(canonical_clickhouse_function_name("sum"), None);
    }

    #[test]
    fn clickhouse_function_casing_entries_are_sorted() {
        for pair in CLICKHOUSE_FUNCTION_CASING.windows(2) {
            assert!(pair[0].0 <= pair[1].0, "entries must be sorted");
        }
    }

    #[test]
    fn clickhouse_case_insensitive_lookup_works() {
        assert!(is_clickhouse_case_insensitive_function("ANY_VALUE"));
        assert!(is_clickhouse_case_insensitive_function(
            "first_value_respect_nulls"
        ));
        assert!(is_clickhouse_case_insensitive_function("AVG"));
        assert!(!is_clickhouse_case_insensitive_function("anyRespectNulls"));
    }

    #[test]
    fn clickhouse_case_insensitive_entries_are_sorted() {
        for pair in CLICKHOUSE_CASE_INSENSITIVE_FUNCTIONS.windows(2) {
            assert!(pair[0] <= pair[1], "entries must be sorted");
        }
    }
}
