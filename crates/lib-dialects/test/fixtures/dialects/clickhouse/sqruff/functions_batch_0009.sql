-- seq_id 401 | enabledRoles
SELECT enabledRoles();

-- seq_id 402 | encodeURLComponent
SELECT encodeURLComponent('a b');

-- seq_id 403 | encodeURLFormComponent
SELECT encodeURLFormComponent('a+b');

-- seq_id 404 | encodeXMLComponent
SELECT encodeXMLComponent('<tag>');

-- seq_id 405 | encrypt
SELECT encrypt('aes-128-ecb', unhex('000102030405060708090A0B0C0D0E0F'), unhex('00112233445566778899AABBCCDDEEFF'));

-- seq_id 406 | endsWith
SELECT endsWith('abc', 'bc');

-- seq_id 407 | endsWithUTF8
SELECT endsWithUTF8('富强民主文明和谐', '富强');

-- seq_id 408 | entropy
SELECT entropy('hello');

-- seq_id 409 | equals
SELECT equals(1, 1);

-- seq_id 410 | erf
SELECT erf(1);

-- seq_id 411 | erfc
SELECT erfc(1);

-- seq_id 412 | errorCodeToName
SELECT errorCodeToName(60);

-- seq_id 413 | estimateCompressionRatio
SELECT estimateCompressionRatio('hello world');

-- seq_id 414 | evalMLMethod
SELECT evalMLMethod('method', [1, 2, 3]);

-- seq_id 415 | exp
SELECT exp(1);

-- seq_id 416 | exp10
SELECT exp10(1);

-- seq_id 417 | exp2
SELECT exp2(1);

-- seq_id 418 | exponentialMovingAverage
SELECT exponentialMovingAverage(1);

-- seq_id 419 | exponentialTimeDecayedAvg
SELECT exponentialTimeDecayedAvg(1, 1);

-- seq_id 420 | exponentialTimeDecayedCount
SELECT exponentialTimeDecayedCount(1, 1);

-- seq_id 421 | exponentialTimeDecayedMax
SELECT exponentialTimeDecayedMax(1, 1);

-- seq_id 422 | exponentialTimeDecayedSum
SELECT exponentialTimeDecayedSum(1, 1);

-- seq_id 423 | extract
SELECT extract('([0-9]+)', 'abc123');

-- seq_id 424 | extractAll
SELECT extractAll('([0-9]+)', 'abc123');

-- seq_id 425 | extractAllGroupsHorizontal
SELECT extractAllGroupsHorizontal('([a-z]+)=([0-9]+)', 'a=1,b=2');

-- seq_id 426 | extractAllGroups
SELECT extractAllGroups('([a-z]+)=([0-9]+)', 'a=1,b=2');

-- seq_id 427 | extractAllGroupsVertical
SELECT extractAllGroupsVertical('([a-z]+)=([0-9]+)', 'a=1,b=2');

-- seq_id 428 | extractGroups
SELECT extractGroups('([a-z]+)=([0-9]+)', 'a=1,b=2');

-- seq_id 429 | extractKeyValuePairs
SELECT extractKeyValuePairs('a=1,b=2');

-- seq_id 430 | mapFromString
SELECT mapFromString('a=1,b=2');

-- seq_id 431 | str_to_map
SELECT str_to_map('a=1,b=2');

-- seq_id 432 | extractKeyValuePairsWithEscaping
SELECT extractKeyValuePairsWithEscaping('a=1\\,2,b=3');

-- seq_id 433 | extractTextFromHTML
SELECT extractTextFromHTML('<p>Hello</p>');

-- seq_id 434 | extractURLParameter
SELECT extractURLParameter('https://example.com?a=1&b=2', 'a');

-- seq_id 435 | extractURLParameterNames
SELECT extractURLParameterNames('https://example.com?a=1&b=2');

-- seq_id 436 | extractURLParameters
SELECT extractURLParameters('https://example.com?a=1&b=2');

-- seq_id 437 | factorial
SELECT factorial(10);

-- seq_id 438 | farmFingerprint64
SELECT farmFingerprint64('abc');

-- seq_id 439 | farmHash64
SELECT farmHash64('abc');

-- seq_id 440 | file
SELECT file('path', 'CSV', 'x UInt8');

-- seq_id 441 | filesystemAvailable
SELECT filesystemAvailable();

-- seq_id 442 | filesystemCapacity
SELECT filesystemCapacity();

-- seq_id 443 | filesystemUnreserved
SELECT filesystemUnreserved();

-- seq_id 444 | finalizeAggregation
SELECT finalizeAggregation(sumState(toUInt64(1)));

-- seq_id 445 | firstLine
SELECT firstLine('Hello\nWorld');

-- seq_id 446 | firstSignificantSubdomain
SELECT firstSignificantSubdomain('https://news.clickhouse.com/');

-- seq_id 447 | firstSignificantSubdomainCustom
SELECT firstSignificantSubdomainCustom('https://news.clickhouse.com/', 'com');

-- seq_id 448 | firstSignificantSubdomainCustomRFC
SELECT firstSignificantSubdomainCustomRFC('https://news.clickhouse.com/', 'com');

-- seq_id 449 | firstSignificantSubdomainRFC
SELECT firstSignificantSubdomainRFC('https://news.clickhouse.com/');

-- seq_id 450 | flameGraph
SELECT flameGraph([1, 2, 3]);
