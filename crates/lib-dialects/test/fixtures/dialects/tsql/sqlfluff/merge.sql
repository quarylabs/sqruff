MERGE INTO dbo.target
USING  (	SELECT 1 AS i	) AS source
ON source.i = target.i
WHEN MATCHED
THEN  UPDATE SET target.i = source.i;

MERGE INTO dbo.target AS tgt
USING (SELECT 1 AS i) AS src(i)
ON src.i = tgt.i
WHEN MATCHED
THEN UPDATE SET tgt.i = src.i;
