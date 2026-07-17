SELECT * APPLY(sum) FROM t1;
SELECT * EXCEPT (c1) APPLY(sum) FROM t1;
SELECT * APPLY(col -> sum(col)) FROM t1;
SELECT * APPLY(sum) APPLY(length) APPLY(max) FROM t1;
