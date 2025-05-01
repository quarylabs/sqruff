INSERT INTO target BY NAME SELECT named_struct('b', 2, 'a', 1) AS s, 0 AS n, 'data' AS text;
INSERT INTO target BY NAME SELECT array(named_struct('b', 2, 'a', 1)) AS arr, 0 AS n;
INSERT INTO target BY NAME SELECT array(named_struct('b', 2, 'a', 1)) AS arr;
INSERT INTO target BY NAME SELECT array(named_struct('b', 2, 'a', 1)) AS arr, 0 AS badname;
INSERT INTO target BY NAME SELECT array(named_struct('b', 2, 'a', 1)) AS arr, 0 AS n, 1 AS n;
