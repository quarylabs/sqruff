MERGE INTO dbo.target
USING  (	SELECT 1 AS i	) AS source
ON source.i = target.i
WHEN MATCHED
THEN  UPDATE SET target.i = source.i;
