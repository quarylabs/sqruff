-- seq_id 201 | buildId
SELECT buildId();

-- seq_id 202 | byteHammingDistance
SELECT byteHammingDistance(1, 0);

-- seq_id 203 | mismatches
SELECT mismatches('ab', 'ac');

-- seq_id 204 | byteSize
SELECT byteSize(1);

-- seq_id 205 | byteSwap
SELECT byteSwap(54);

-- seq_id 206 | caseWithExpr
SELECT caseWithExpr(1, 1, 'one', 'other');

-- seq_id 207 | caseWithExpression
SELECT caseWithExpression(1, 1, 'one', 'other');

-- seq_id 208 | CAST
SELECT CAST(1, 'UInt8');

-- seq_id 209 | catboostEvaluate
SELECT catboostEvaluate('model', [1.0], [1.0]);

-- seq_id 210 | categoricalInformationValue
SELECT categoricalInformationValue(1, 1);

-- seq_id 211 | cbrt
SELECT cbrt(8);

-- seq_id 212 | ceil
SELECT ceil(1.5);

-- seq_id 213 | ceiling
SELECT ceiling(1.5);

-- seq_id 214 | changeDay
SELECT changeDay(toDate('2024-01-15'), 1);

-- seq_id 215 | changeHour
SELECT changeHour(toDateTime('2024-01-15 12:00:00'), 1);

-- seq_id 216 | changeMinute
SELECT changeMinute(toDateTime('2024-01-15 12:34:00'), 1);

-- seq_id 217 | changeMonth
SELECT changeMonth(toDate('2024-01-15'), 2);

-- seq_id 218 | changeSecond
SELECT changeSecond(toDateTime('2024-01-15 12:34:56'), 1);

-- seq_id 219 | changeYear
SELECT changeYear(toDate('2024-01-15'), 2025);

-- seq_id 220 | char
SELECT char(65);

-- seq_id 221 | cityHash64
SELECT cityHash64('abc');

-- seq_id 222 | clamp
SELECT clamp(5, 1, 10);

-- seq_id 223 | coalesce
SELECT coalesce(NULL, 1);

-- seq_id 224 | compareSubstrings
SELECT compareSubstrings('123', '123', 0, 0, 3);

-- seq_id 225 | concat
SELECT concat('a', 'b');

-- seq_id 226 | concatAssumeInjective
SELECT concatAssumeInjective('a', 'b');

-- seq_id 227 | concat_ws
SELECT concat_ws('-', 'a', 'b');

-- seq_id 228 | concatWithSeparator
SELECT concatWithSeparator('-', 'a', 'b');

-- seq_id 229 | concatWithSeparatorAssumeInjective
SELECT concatWithSeparatorAssumeInjective('-', 'a', 'b');

-- seq_id 230 | connection_id
SELECT connection_id();

-- seq_id 231 | connectionId
SELECT connectionId();

-- seq_id 232 | contingency
SELECT contingency(1, 1);

-- seq_id 233 | convertCharset
SELECT convertCharset('abc', 'UTF-8', 'UTF-8');

-- seq_id 234 | corr
SELECT corr(1, 2);

-- seq_id 235 | corrMatrix
SELECT corrMatrix(1, 2);

-- seq_id 236 | corrStable
SELECT corrStable(1, 2);

-- seq_id 237 | cos
SELECT cos(1);

-- seq_id 238 | cosh
SELECT cosh(1);

-- seq_id 239 | cosineDistance
SELECT cosineDistance([1.0], [1.0]);

-- seq_id 240 | count
SELECT count();

-- seq_id 241 | countDigits
SELECT countDigits(12345);

-- seq_id 242 | countEqual
SELECT countEqual([1, 2, 1], 1);

-- seq_id 243 | countMatches
SELECT countMatches('abab', 'ab');

-- seq_id 244 | countMatchesCaseInsensitive
SELECT countMatchesCaseInsensitive('AbAb', 'ab');

-- seq_id 245 | countSubstrings
SELECT countSubstrings('abab', 'ab');

-- seq_id 246 | countSubstringsCaseInsensitive
SELECT countSubstringsCaseInsensitive('AbAb', 'ab');

-- seq_id 247 | countSubstringsCaseInsensitiveUTF8
SELECT countSubstringsCaseInsensitiveUTF8('AbAb', 'ab');

-- seq_id 248 | COVAR_POP
SELECT COVAR_POP(1, 2);

-- seq_id 249 | covarPop
SELECT covarPop(1, 2);

-- seq_id 250 | covarPopMatrix
SELECT covarPopMatrix(1, 2);
