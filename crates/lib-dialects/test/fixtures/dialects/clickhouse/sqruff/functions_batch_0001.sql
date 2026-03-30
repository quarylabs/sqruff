-- seq_id 1 | __actionName
SELECT __actionName(1);

-- seq_id 2 | __bitBoolMaskAnd
SELECT __bitBoolMaskAnd(1);

-- seq_id 3 | __bitBoolMaskOr
SELECT __bitBoolMaskOr(1);

-- seq_id 4 | __bitSwapLastTwo
SELECT __bitSwapLastTwo(1);

-- seq_id 5 | __bitWrapperFunc
SELECT __bitWrapperFunc(1);

-- seq_id 6 | __getScalar
SELECT __getScalar(1);

-- seq_id 7 | __scalarSubqueryResult
SELECT __scalarSubqueryResult(1);

-- seq_id 8 | _CAST
SELECT _CAST(1, 'UInt8');

-- seq_id 9 | abs
SELECT abs(1);

-- seq_id 10 | accurateCast
SELECT accurateCast(1);

-- seq_id 11 | accurateCastOrDefault
SELECT accurateCastOrDefault(1);

-- seq_id 12 | accurateCastOrNull
SELECT accurateCastOrNull(1);

-- seq_id 13 | acos
SELECT acos(1);

-- seq_id 14 | acosh
SELECT acosh(1);

-- seq_id 15 | addDate
SELECT addDate(1);

-- seq_id 16 | addDays
SELECT addDays(1);

-- seq_id 17 | addHours
SELECT addHours(1);

-- seq_id 18 | addInterval
SELECT addInterval((INTERVAL 1 DAY, INTERVAL 1 YEAR), INTERVAL 1 MONTH);

-- seq_id 19 | addMicroseconds
SELECT addMicroseconds(1);

-- seq_id 20 | addMilliseconds
SELECT addMilliseconds(1);

-- seq_id 21 | addMinutes
SELECT addMinutes(1);

-- seq_id 22 | addMonths
SELECT addMonths(1);

-- seq_id 23 | addNanoseconds
SELECT addNanoseconds(1);

-- seq_id 24 | addQuarters
SELECT addQuarters(1);

-- seq_id 25 | addressToLine
SELECT addressToLine(1);

-- seq_id 26 | addressToLineWithInlines
SELECT addressToLineWithInlines(1);

-- seq_id 27 | addressToSymbol
SELECT addressToSymbol(1);

-- seq_id 28 | addSeconds
SELECT addSeconds(1);

-- seq_id 29 | addTupleOfIntervals
WITH toDate('2018-01-01') AS date SELECT addTupleOfIntervals(date, (INTERVAL 1 DAY, INTERVAL 1 YEAR));

-- seq_id 30 | addWeeks
SELECT addWeeks(1);

-- seq_id 31 | addYears
SELECT addYears(1);

-- seq_id 32 | aes_decrypt_mysql
SELECT aes_decrypt_mysql(1);

-- seq_id 33 | aes_encrypt_mysql
SELECT aes_encrypt_mysql(1);

-- seq_id 34 | age
SELECT age(1);

-- seq_id 35 | aggThrow
SELECT aggThrow(1);

-- seq_id 36 | alphaTokens
SELECT alphaTokens(1);

-- seq_id 37 | splitByAlpha
SELECT splitByAlpha(1);

-- seq_id 38 | analysisOfVariance
SELECT analysisOfVariance(1);

-- seq_id 39 | anova
SELECT anova(1);

-- seq_id 40 | and
SELECT and(1, 1);

-- seq_id 41 | any
SELECT any(1);

-- seq_id 42 | any_value
SELECT any_value(1);

-- seq_id 43 | first_value
SELECT first_value(1);

-- seq_id 44 | any_respect_nulls
SELECT any_respect_nulls(1);

-- seq_id 45 | any_value_respect_nulls
SELECT any_value_respect_nulls(1);

-- seq_id 46 | anyRespectNulls
SELECT anyRespectNulls(1);

-- seq_id 47 | anyValueRespectNulls
SELECT anyValueRespectNulls(1);

-- seq_id 48 | first_value_respect_nulls
SELECT first_value_respect_nulls(1);

-- seq_id 49 | firstValueRespectNulls
SELECT firstValueRespectNulls(1);

-- seq_id 50 | anyHeavy
SELECT anyHeavy(1);
