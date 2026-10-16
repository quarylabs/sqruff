SELECT COUNT(prior.id) AS n
FROM foo AS prior
WHERE prior.id = 1;
SELECT prior.id
FROM foo AS prior;
SELECT prior.*
FROM foo AS prior;
