CREATE TABLE #procs
WITH (DISTRIBUTION = HASH([eid])) AS
WITH proc_icd
AS
( SELECT
    *
  FROM fbp
)
SELECT
   *
FROM
(
   SELECT
       *
   FROM proc_icd
) sub
;