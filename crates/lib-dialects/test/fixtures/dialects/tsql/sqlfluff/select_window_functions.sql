SELECT
    ROW_NUMBER()OVER(PARTITION BY [EventNM], [PersonID] ORDER BY [DateofEvent] desc) AS [RN],
    RANK()OVER(PARTITION BY [EventNM] ORDER BY [DateofEvent] desc) AS [R],
    DENSE_RANK()OVER(PARTITION BY [EventNM] ORDER BY [DateofEvent] desc) AS [DR],
    NTILE(5)OVER(PARTITION BY [EventNM] ORDER BY [DateofEvent] desc) AS [NT],
	sum(t.col1) over (partition by t.col2, t.col3),

	ROW_NUMBER() OVER (PARTITION BY (SELECT mediaty FROM dbo.MediaTypes ms WHERE ms.MediaTypeID = f.mediatypeid) ORDER BY AdjustedPriorityScore DESC) AS Subselect_Partition,

	ROW_NUMBER() OVER (PARTITION BY COALESCE(NPI1, NPI2) ORDER BY COALESCE(SystemEffectiveDTS1, SystemEffectiveDTS2) DESC) AS Coalesce_Partition,

	ROW_NUMBER() OVER (PARTITION BY (DayInMonth), (DaySuffix) ORDER BY Month ASC),
	COUNT(*) OVER (PARTITION BY NULL),

	[preceding]	= count(*) over(order by object_id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW ),
	[central]	= count(*) over(order by object_id ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING ),
	[following]	= count(*) over(order by object_id ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING)
FROM dbo.all_pop;
