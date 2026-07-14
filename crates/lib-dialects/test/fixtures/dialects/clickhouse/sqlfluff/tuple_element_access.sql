SELECT
  tuple(1, 2, 3) AS arr,
  arr.1 AS dsa,
  arr[1].2[3] AS mixed_array_tuple_access,
  tuple(1, 2).1 AS first_tuple_value;

WITH tuple(tuple(tuple('a', 'aa'), 'b'), 'c') AS test
SELECT
    test.1.1.2,
    (test.1).2,
    ((test.1).2).3,
    test.1[2],
    test.1[2].3,
    test.2;
