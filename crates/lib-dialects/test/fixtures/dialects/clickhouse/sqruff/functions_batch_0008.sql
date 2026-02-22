-- seq_id 351 | dictGetString
SELECT dictGetString('my_dict', 'my_attr', toUInt64(1));

-- seq_id 352 | dictGetStringOrDefault
SELECT dictGetStringOrDefault('my_dict', 'my_attr', toUInt64(1), 'default');

-- seq_id 353 | dictGetUInt16
SELECT dictGetUInt16('my_dict', 'my_attr', toUInt64(1));

-- seq_id 354 | dictGetUInt16OrDefault
SELECT dictGetUInt16OrDefault('my_dict', 'my_attr', toUInt64(1), toUInt16(0));

-- seq_id 355 | dictGetUInt32
SELECT dictGetUInt32('my_dict', 'my_attr', toUInt64(1));

-- seq_id 356 | dictGetUInt32OrDefault
SELECT dictGetUInt32OrDefault('my_dict', 'my_attr', toUInt64(1), toUInt32(0));

-- seq_id 357 | dictGetUInt64
SELECT dictGetUInt64('my_dict', 'my_attr', toUInt64(1));

-- seq_id 358 | dictGetUInt64OrDefault
SELECT dictGetUInt64OrDefault('my_dict', 'my_attr', toUInt64(1), toUInt64(0));

-- seq_id 359 | dictGetUInt8
SELECT dictGetUInt8('my_dict', 'my_attr', toUInt64(1));

-- seq_id 360 | dictGetUInt8OrDefault
SELECT dictGetUInt8OrDefault('my_dict', 'my_attr', toUInt64(1), toUInt8(0));

-- seq_id 361 | dictGetUUID
SELECT dictGetUUID('my_dict', 'my_attr', toUInt64(1));

-- seq_id 362 | dictGetUUIDOrDefault
SELECT dictGetUUIDOrDefault('my_dict', 'my_attr', toUInt64(1), toUUID('00000000-0000-0000-0000-000000000000'));

-- seq_id 363 | dictHas
SELECT dictHas('my_dict', toUInt64(1));

-- seq_id 364 | dictIsIn
SELECT dictIsIn('my_dict', toUInt64(1), toUInt64(2));

-- seq_id 365 | displayName
SELECT displayName();

-- seq_id 366 | distinctDynamicTypes
SELECT distinctDynamicTypes(d) FROM t;

-- seq_id 367 | distinctJSONPaths
SELECT distinctJSONPaths(d) FROM t;

-- seq_id 368 | distinctJSONPathsAndTypes
SELECT distinctJSONPathsAndTypes(d) FROM t;

-- seq_id 369 | divide
SELECT divide(6, 2);

-- seq_id 370 | divideDecimal
SELECT divideDecimal(toDecimal64(-12, 1), toDecimal32(2.1, 1), 5);

-- seq_id 371 | domain
SELECT domain('svn+ssh://some.svn-hosting.com:80/repo/trunk');

-- seq_id 372 | domainRFC
SELECT domainRFC('https://www.clickhouse.com');

-- seq_id 373 | domainWithoutWWW
SELECT domainWithoutWWW('https://www.clickhouse.com');

-- seq_id 374 | domainWithoutWWWRFC
SELECT domainWithoutWWWRFC('https://www.clickhouse.com');

-- seq_id 375 | dotProduct
SELECT dotProduct([1, 2], [3, 4]);

-- seq_id 376 | scalarProduct
SELECT scalarProduct([1, 2], [3, 4]);

-- seq_id 377 | dumpColumnStructure
SELECT dumpColumnStructure(tuple(1, 'a'));

-- seq_id 378 | dynamicElement
SELECT dynamicElement(d, 'String') FROM t;

-- seq_id 379 | dynamicType
SELECT dynamicType(d) FROM t;

-- seq_id 380 | e
SELECT e();

-- seq_id 381 | editDistance
SELECT editDistance('abc', 'adc');

-- seq_id 382 | levenshteinDistance
SELECT levenshteinDistance('abc', 'adc');

-- seq_id 383 | editDistanceUTF8
SELECT editDistanceUTF8('abc', 'adc');

-- seq_id 384 | levenshteinDistanceUTF8
SELECT levenshteinDistanceUTF8('abc', 'adc');

-- seq_id 385 | empty
SELECT empty([1]);

-- seq_id 386 | emptyArrayDate
SELECT emptyArrayDate();

-- seq_id 387 | emptyArrayDateTime
SELECT emptyArrayDateTime();

-- seq_id 388 | emptyArrayFloat32
SELECT emptyArrayFloat32();

-- seq_id 389 | emptyArrayFloat64
SELECT emptyArrayFloat64();

-- seq_id 390 | emptyArrayInt16
SELECT emptyArrayInt16();

-- seq_id 391 | emptyArrayInt32
SELECT emptyArrayInt32();

-- seq_id 392 | emptyArrayInt64
SELECT emptyArrayInt64();

-- seq_id 393 | emptyArrayInt8
SELECT emptyArrayInt8();

-- seq_id 394 | emptyArrayString
SELECT emptyArrayString();

-- seq_id 395 | emptyArrayToSingle
SELECT emptyArrayToSingle(emptyArrayInt8());

-- seq_id 396 | emptyArrayUInt16
SELECT emptyArrayUInt16();

-- seq_id 397 | emptyArrayUInt32
SELECT emptyArrayUInt32();

-- seq_id 398 | emptyArrayUInt64
SELECT emptyArrayUInt64();

-- seq_id 399 | emptyArrayUInt8
SELECT emptyArrayUInt8();

-- seq_id 400 | enabledProfiles
SELECT enabledProfiles();
