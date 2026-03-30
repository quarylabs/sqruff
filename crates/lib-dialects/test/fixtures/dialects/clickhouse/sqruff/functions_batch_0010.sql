-- seq_id 451 | flattenTuple
SELECT flattenTuple(1);

-- seq_id 452 | floor
SELECT floor(1);

-- seq_id 453 | format
SELECT *
FROM format(
    JSONEachRow,
    '\n{"a": "Hello", "b": 111}\n{"a": "World", "b": 123}\n{"a": "Hello", "b": 112}\n{"a": "World", "b": 124}\n'
);

-- seq_id 454 | DATE_FORMAT
SELECT DATE_FORMAT(1);

-- seq_id 455 | formatDateTime
SELECT formatDateTime(1);

-- seq_id 456 | formatDateTimeInJodaSyntax
SELECT formatDateTimeInJodaSyntax(1);

-- seq_id 457 | formatQuery
SELECT formatQuery(1);

-- seq_id 458 | formatQueryOrNull
SELECT formatQueryOrNull(1);

-- seq_id 459 | formatQuerySingleLine
SELECT formatQuerySingleLine(1);

-- seq_id 460 | formatQuerySingleLineOrNull
SELECT formatQuerySingleLineOrNull(1);

-- seq_id 461 | formatReadableDecimalSize
SELECT formatReadableDecimalSize(1);

-- seq_id 462 | formatReadableQuantity
SELECT formatReadableQuantity(1);

-- seq_id 463 | FORMAT_BYTES
SELECT FORMAT_BYTES(1);

-- seq_id 464 | formatReadableSize
SELECT formatReadableSize(1);

-- seq_id 465 | formatReadableTimeDelta
SELECT formatReadableTimeDelta(1);

-- seq_id 466 | formatRow
SELECT formatRow(1);

-- seq_id 467 | formatRowNoNewline
SELECT formatRowNoNewline(1);

-- seq_id 468 | FQDN
SELECT FQDN(1);

-- seq_id 469 | fullHostName
SELECT fullHostName(1);

-- seq_id 470 | fragment
SELECT fragment(1);

-- seq_id 471 | FROM_DAYS
SELECT FROM_DAYS(1);

-- seq_id 472 | fromDaysSinceYearZero
SELECT fromDaysSinceYearZero(1);

-- seq_id 473 | fromDaysSinceYearZero32
SELECT fromDaysSinceYearZero32(1);

-- seq_id 474 | fromModifiedJulianDay
SELECT fromModifiedJulianDay(1);

-- seq_id 475 | fromModifiedJulianDayOrNull
SELECT fromModifiedJulianDayOrNull(1);

-- seq_id 476 | FROM_UNIXTIME
SELECT FROM_UNIXTIME(1);

-- seq_id 477 | fromUnixTimestamp
SELECT fromUnixTimestamp(1);

-- seq_id 478 | fromUnixTimestamp64Micro
SELECT fromUnixTimestamp64Micro(1);

-- seq_id 479 | fromUnixTimestamp64Milli
SELECT fromUnixTimestamp64Milli(1);

-- seq_id 480 | fromUnixTimestamp64Nano
SELECT fromUnixTimestamp64Nano(1);

-- seq_id 481 | fromUnixTimestamp64Second
SELECT fromUnixTimestamp64Second(1);

-- seq_id 482 | fromUnixTimestampInJodaSyntax
SELECT fromUnixTimestampInJodaSyntax(1);

-- seq_id 483 | from_utc_timestamp
SELECT from_utc_timestamp(1);

-- seq_id 484 | fromUTCTimestamp
SELECT fromUTCTimestamp(1);

-- seq_id 485 | fuzzBits
SELECT fuzzBits(1);

-- seq_id 486 | gccMurmurHash
SELECT gccMurmurHash(1);

-- seq_id 487 | gcd
SELECT gcd(1);

-- seq_id 488 | generateRandomStructure
SELECT generateRandomStructure(1);

-- seq_id 489 | generateSerialID
SELECT generateSerialID(1);

-- seq_id 490 | generateSnowflakeID
SELECT generateSnowflakeID(1);

-- seq_id 491 | generateULID
SELECT generateULID(1);

-- seq_id 492 | generateUUIDv4
SELECT generateUUIDv4(1);

-- seq_id 493 | generateUUIDv7
SELECT generateUUIDv7(1);

-- seq_id 494 | geoDistance
SELECT geoDistance(1);

-- seq_id 495 | geohashDecode
SELECT geohashDecode(1);

-- seq_id 496 | geohashEncode
SELECT geohashEncode(1);

-- seq_id 497 | geohashesInBox
SELECT geohashesInBox(1);

-- seq_id 498 | geoToH3
SELECT geoToH3(1);

-- seq_id 499 | geoToS2
SELECT geoToS2(1);

-- seq_id 500 | getClientHTTPHeader
SELECT getClientHTTPHeader(1);
