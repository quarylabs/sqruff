SELECT a FROM t WHERE a == 1;

SELECT a == b AS same_value FROM t;

SELECT count() FROM t WHERE number % 2 == 0;

SELECT a FROM t WHERE a IS NOT DISTINCT FROM b;

SELECT a == b ? 1 : 0 AS same_value FROM t;
