-- seq_id 301 | DATE_TRUNC
SELECT DATE_TRUNC('day', now());

-- seq_id 302 | dateTrunc
SELECT dateTrunc('day', now());

-- seq_id 303 | decodeHTMLComponent
SELECT decodeHTMLComponent('&lt;tag&gt;');

-- seq_id 304 | decodeURLComponent
SELECT decodeURLComponent('https%3A%2F%2Fexample.com%2Fa%20b');

-- seq_id 305 | decodeURLFormComponent
SELECT decodeURLFormComponent('a%2Bb');

-- seq_id 306 | decodeXMLComponent
SELECT decodeXMLComponent('&lt;tag&gt;');

-- seq_id 307 | decrypt
SELECT decrypt('aes-128-ecb', unhex('000102030405060708090A0B0C0D0E0F'), unhex('00112233445566778899AABBCCDDEEFF'));

-- seq_id 308 | defaultProfiles
SELECT defaultProfiles();

-- seq_id 309 | defaultRoles
SELECT defaultRoles();

-- seq_id 310 | defaultValueOfArgumentType
SELECT defaultValueOfArgumentType('UInt8');

-- seq_id 311 | defaultValueOfTypeName
SELECT defaultValueOfTypeName('UInt8');

-- seq_id 312 | degrees
SELECT degrees(1);

-- seq_id 313 | deltaSum
SELECT deltaSum(1);

-- seq_id 314 | deltaSumTimestamp
SELECT deltaSumTimestamp(1, toDateTime('2024-01-01 00:00:00'));

-- seq_id 315 | demangle
SELECT demangle('_Z3foov');

-- seq_id 316 | dense_rank
SELECT dense_rank() OVER (ORDER BY number) FROM numbers(1);

-- seq_id 317 | denseRank
SELECT denseRank() OVER (ORDER BY number) FROM numbers(1);

-- seq_id 318 | detectCharset
SELECT detectCharset('hello world');

-- seq_id 319 | detectLanguage
SELECT detectLanguage('hello world');

-- seq_id 320 | detectLanguageMixed
SELECT detectLanguageMixed('hello world');

-- seq_id 321 | detectLanguageUnknown
SELECT detectLanguageUnknown('hello world');

-- seq_id 322 | detectProgrammingLanguage
SELECT detectProgrammingLanguage('fn main() {}');

-- seq_id 323 | detectTonality
SELECT detectTonality('great');

-- seq_id 324 | dictGet
SELECT dictGet('my_dict', 'my_attr', toUInt64(1));

-- seq_id 325 | dictGetAll
SELECT dictGetAll('my_dict', 'my_attr', toUInt64(1), 10);

-- seq_id 326 | dictGetChildren
SELECT dictGetChildren('my_dict', toUInt64(1));

-- seq_id 327 | dictGetDate
SELECT dictGetDate('my_dict', 'my_attr', toUInt64(1));

-- seq_id 328 | dictGetDateOrDefault
SELECT dictGetDateOrDefault('my_dict', 'my_attr', toUInt64(1), toDate('2024-01-01'));

-- seq_id 329 | dictGetDateTime
SELECT dictGetDateTime('my_dict', 'my_attr', toUInt64(1));

-- seq_id 330 | dictGetDateTimeOrDefault
SELECT dictGetDateTimeOrDefault('my_dict', 'my_attr', toUInt64(1), toDateTime('2024-01-01 00:00:00'));

-- seq_id 331 | dictGetDescendants
SELECT dictGetDescendants('my_dict', toUInt64(1));

-- seq_id 332 | dictGetFloat32
SELECT dictGetFloat32('my_dict', 'my_attr', toUInt64(1));

-- seq_id 333 | dictGetFloat32OrDefault
SELECT dictGetFloat32OrDefault('my_dict', 'my_attr', toUInt64(1), toFloat32(0));

-- seq_id 334 | dictGetFloat64
SELECT dictGetFloat64('my_dict', 'my_attr', toUInt64(1));

-- seq_id 335 | dictGetFloat64OrDefault
SELECT dictGetFloat64OrDefault('my_dict', 'my_attr', toUInt64(1), toFloat64(0));

-- seq_id 336 | dictGetHierarchy
SELECT dictGetHierarchy('my_dict', toUInt64(1));

-- seq_id 337 | dictGetInt16
SELECT dictGetInt16('my_dict', 'my_attr', toUInt64(1));

-- seq_id 338 | dictGetInt16OrDefault
SELECT dictGetInt16OrDefault('my_dict', 'my_attr', toUInt64(1), toInt16(0));

-- seq_id 339 | dictGetInt32
SELECT dictGetInt32('my_dict', 'my_attr', toUInt64(1));

-- seq_id 340 | dictGetInt32OrDefault
SELECT dictGetInt32OrDefault('my_dict', 'my_attr', toUInt64(1), toInt32(0));

-- seq_id 341 | dictGetInt64
SELECT dictGetInt64('my_dict', 'my_attr', toUInt64(1));

-- seq_id 342 | dictGetInt64OrDefault
SELECT dictGetInt64OrDefault('my_dict', 'my_attr', toUInt64(1), toInt64(0));

-- seq_id 343 | dictGetInt8
SELECT dictGetInt8('my_dict', 'my_attr', toUInt64(1));

-- seq_id 344 | dictGetInt8OrDefault
SELECT dictGetInt8OrDefault('my_dict', 'my_attr', toUInt64(1), toInt8(0));

-- seq_id 345 | dictGetIPv4
SELECT dictGetIPv4('my_dict', 'my_attr', toUInt64(1));

-- seq_id 346 | dictGetIPv4OrDefault
SELECT dictGetIPv4OrDefault('my_dict', 'my_attr', toUInt64(1), toIPv4('0.0.0.0'));

-- seq_id 347 | dictGetIPv6
SELECT dictGetIPv6('my_dict', 'my_attr', toUInt64(1));

-- seq_id 348 | dictGetIPv6OrDefault
SELECT dictGetIPv6OrDefault('my_dict', 'my_attr', toUInt64(1), toIPv6('::'));

-- seq_id 349 | dictGetOrDefault
SELECT dictGetOrDefault('my_dict', 'my_attr', toUInt64(1), toUInt64(0));

-- seq_id 350 | dictGetOrNull
SELECT dictGetOrNull('my_dict', 'my_attr', toUInt64(1));
