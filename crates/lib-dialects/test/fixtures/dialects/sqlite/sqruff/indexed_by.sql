SELECT email FROM example INDEXED BY example_email;

SELECT email FROM example AS e INDEXED BY main.example_email WHERE email > 'a';

SELECT email FROM example e NOT INDEXED;

SELECT *
FROM a INDEXED BY a_idx
JOIN b NOT INDEXED ON a.id = b.id
JOIN c AS c1 INDEXED BY c_idx ON c1.id = b.id;

UPDATE example INDEXED BY example_email SET email = 'b' WHERE email = 'a';

UPDATE example AS e NOT INDEXED SET email = 'b';

DELETE FROM example INDEXED BY example_email WHERE email = 'a';

DELETE FROM example NOT INDEXED WHERE email = 'a';
