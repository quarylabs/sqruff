-- seq_id 51 | anyLast
SELECT anyLast(1);

-- seq_id 52 | last_value
SELECT last_value(1);

-- seq_id 53 | anyLast_respect_nulls
SELECT anyLast_respect_nulls(1);

-- seq_id 54 | anyLastRespectNulls
SELECT anyLastRespectNulls(1);

-- seq_id 55 | last_value_respect_nulls
SELECT last_value_respect_nulls(1);

-- seq_id 56 | lastValueRespectNulls
SELECT lastValueRespectNulls(1);

-- seq_id 57 | appendTrailingCharIfAbsent
SELECT appendTrailingCharIfAbsent(1);

-- seq_id 58 | approx_top_count
SELECT approx_top_count(1);

-- seq_id 59 | approx_top_k
SELECT approx_top_k(1);

-- seq_id 60 | approx_top_sum
SELECT approx_top_sum(1);

-- seq_id 61 | argMax
SELECT argMax(1);

-- seq_id 62 | argMin
SELECT argMin(1);

-- seq_id 63 | array
SELECT array(1, 2);

-- seq_id 64 | arrayAll
SELECT arrayAll(1);

-- seq_id 65 | arrayAUCPR
SELECT arrayAUCPR(1);

-- seq_id 66 | arrayPRAUC
SELECT arrayPRAUC(1);

-- seq_id 67 | arrayAvg
SELECT arrayAvg(1);

-- seq_id 68 | arrayCompact
SELECT arrayCompact(1);

-- seq_id 69 | arrayConcat
SELECT arrayConcat(1);

-- seq_id 70 | arrayCount
SELECT arrayCount(1);

-- seq_id 71 | arrayCumSum
SELECT arrayCumSum(1);

-- seq_id 72 | arrayCumSumNonNegative
SELECT arrayCumSumNonNegative(1);

-- seq_id 73 | arrayDifference
SELECT arrayDifference(1);

-- seq_id 74 | arrayDistinct
SELECT arrayDistinct(1);

-- seq_id 75 | arrayDotProduct
SELECT arrayDotProduct(1);

-- seq_id 76 | arrayElement
SELECT arrayElement(1);

-- seq_id 77 | arrayElementOrNull
SELECT arrayElementOrNull(1);

-- seq_id 78 | arrayEnumerate
SELECT arrayEnumerate(1);

-- seq_id 79 | arrayEnumerateDense
SELECT arrayEnumerateDense(1);

-- seq_id 80 | arrayEnumerateDenseRanked
SELECT arrayEnumerateDenseRanked(1);

-- seq_id 81 | arrayEnumerateUniq
SELECT arrayEnumerateUniq(1);

-- seq_id 82 | arrayEnumerateUniqRanked
SELECT arrayEnumerateUniqRanked(1);

-- seq_id 83 | arrayExists
SELECT arrayExists(1);

-- seq_id 84 | arrayFill
SELECT arrayFill(1);

-- seq_id 85 | arrayFilter
SELECT arrayFilter(1);

-- seq_id 86 | arrayFirst
SELECT arrayFirst(1);

-- seq_id 87 | arrayFirstIndex
SELECT arrayFirstIndex(1);

-- seq_id 88 | arrayFirstOrNull
SELECT arrayFirstOrNull(1);

-- seq_id 89 | arrayFlatten
SELECT arrayFlatten(1);

-- seq_id 90 | flatten
SELECT flatten(1);

-- seq_id 91 | arrayFold
SELECT arrayFold(acc, x -> acc + x, [1, 2, 3, 4], toInt64(1));

-- seq_id 92 | arrayIntersect
SELECT arrayIntersect(1);

-- seq_id 93 | arrayJaccardIndex
SELECT arrayJaccardIndex(1);

-- seq_id 94 | arrayJoin
SELECT arrayJoin(1);

-- seq_id 95 | arrayLast
SELECT arrayLast(1);

-- seq_id 96 | arrayLastIndex
SELECT arrayLastIndex(1);

-- seq_id 97 | arrayLastOrNull
SELECT arrayLastOrNull(1);

-- seq_id 98 | arrayLevenshteinDistance
SELECT arrayLevenshteinDistance([1, 2, 4], [1, 2, 3]);

-- seq_id 99 | arrayLevenshteinDistanceWeighted
SELECT arrayLevenshteinDistanceWeighted(
    ['A', 'B', 'C'],
    ['A', 'K', 'L'],
    [1.0, 2, 3],
    [3.0, 4, 5]
);

-- seq_id 100 | arrayMap
SELECT arrayMap(1);
