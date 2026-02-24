-- seq_id 251 | covarPopStable
SELECT covarPopStable(1, 2);

-- seq_id 252 | COVAR_SAMP
SELECT COVAR_SAMP(1, 2);

-- seq_id 253 | covarSamp
SELECT covarSamp(1, 2);

-- seq_id 254 | covarSampMatrix
SELECT covarSampMatrix(1, 2);

-- seq_id 255 | covarSampStable
SELECT covarSampStable(1, 2);

-- seq_id 256 | cramersV
SELECT cramersV(1, 1);

-- seq_id 257 | cramersVBiasCorrected
SELECT cramersVBiasCorrected(1, 1);

-- seq_id 258 | CRC32
SELECT CRC32('abc');

-- seq_id 259 | CRC32IEEE
SELECT CRC32IEEE('abc');

-- seq_id 260 | CRC64
SELECT CRC64('abc');

-- seq_id 261 | current_database
SELECT current_database();

-- seq_id 262 | currentDatabase
SELECT currentDatabase();

-- seq_id 263 | DATABASE
SELECT DATABASE();

-- seq_id 264 | SCHEMA
SELECT SCHEMA();

-- seq_id 265 | currentProfiles
SELECT currentProfiles();

-- seq_id 266 | current_query_id
SELECT current_query_id();

-- seq_id 267 | currentQueryID
SELECT currentQueryID();

-- seq_id 268 | currentRoles
SELECT currentRoles();

-- seq_id 269 | current_schemas
SELECT current_schemas(true);

-- seq_id 270 | currentSchemas
SELECT currentSchemas(true);

-- seq_id 271 | current_user
SELECT current_user();

-- seq_id 272 | currentUser
SELECT currentUser();

-- seq_id 273 | user
SELECT user();

-- seq_id 274 | cutFragment
SELECT cutFragment('https://example.com/path?x=1#frag');

-- seq_id 275 | cutIPv6
SELECT cutIPv6('2001:db8::1', 0, 0);

-- seq_id 276 | cutQueryString
SELECT cutQueryString('https://example.com/path?x=1');

-- seq_id 277 | cutQueryStringAndFragment
SELECT cutQueryStringAndFragment('https://example.com/path?x=1#frag');

-- seq_id 278 | cutToFirstSignificantSubdomain
SELECT cutToFirstSignificantSubdomain('https://news.clickhouse.com.tr/');

-- seq_id 279 | cutToFirstSignificantSubdomainCustom
SELECT cutToFirstSignificantSubdomainCustom('bar.foo.there-is-no-such-domain', 'public_suffix_list');

-- seq_id 280 | cutToFirstSignificantSubdomainCustomRFC
SELECT cutToFirstSignificantSubdomainCustomRFC('bar.foo.there-is-no-such-domain', 'public_suffix_list');

-- seq_id 281 | cutToFirstSignificantSubdomainCustomWithWWW
SELECT cutToFirstSignificantSubdomainCustomWithWWW('www.foo', 'public_suffix_list');

-- seq_id 282 | cutToFirstSignificantSubdomainCustomWithWWWRFC
SELECT cutToFirstSignificantSubdomainCustomWithWWWRFC('www.foo', 'public_suffix_list');

-- seq_id 283 | cutToFirstSignificantSubdomainRFC
SELECT cutToFirstSignificantSubdomainRFC('www.tr');

-- seq_id 284 | cutToFirstSignificantSubdomainWithWWW
SELECT cutToFirstSignificantSubdomainWithWWW('www.tr');

-- seq_id 285 | cutToFirstSignificantSubdomainWithWWWRFC
SELECT cutToFirstSignificantSubdomainWithWWWRFC('www.tr');

-- seq_id 286 | cutURLParameter
SELECT cutURLParameter('https://example.com/path?a=1&b=2', 'a');

-- seq_id 287 | cutWWW
SELECT cutWWW('www.example.com');

-- seq_id 288 | damerauLevenshteinDistance
SELECT damerauLevenshteinDistance('abc', 'acb');

-- seq_id 289 | DATE
SELECT DATE('2024-01-01');

-- seq_id 290 | DATE_DIFF
SELECT DATE_DIFF('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 291 | date_diff
SELECT date_diff('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 292 | dateDiff
SELECT dateDiff('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 293 | TIMESTAMP_DIFF
SELECT TIMESTAMP_DIFF('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 294 | timestamp_diff
SELECT timestamp_diff('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 295 | timestampDiff
SELECT timestampDiff('day', toDate('2024-01-02'), toDate('2024-01-01'));

-- seq_id 296 | dateName
SELECT dateName('month', toDate('2024-01-01'));

-- seq_id 297 | dateTime64ToSnowflake
SELECT dateTime64ToSnowflake(toDateTime64('2021-08-15 18:57:56', 3, 'Asia/Shanghai'));

-- seq_id 298 | dateTime64ToSnowflakeID
SELECT dateTime64ToSnowflakeID(toDateTime64('2021-08-15 18:57:56', 3, 'Asia/Shanghai'));

-- seq_id 299 | dateTimeToSnowflake
SELECT dateTimeToSnowflake(toDateTime('2021-08-15 18:57:56', 'Asia/Shanghai'));

-- seq_id 300 | dateTimeToSnowflakeID
SELECT dateTimeToSnowflakeID(toDateTime('2021-08-15 18:57:56', 'Asia/Shanghai'));
