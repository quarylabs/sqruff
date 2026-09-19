SELECT
    tst1.Name, tst2.OtherName
FROM dbo.Test1 AS tst1
    LEFT OUTER JOIN (dbo.Test2       AS tst2
                          INNER JOIN dbo.FilterTable AS fltr1
                              ON tst2.Id = fltr1.Id)
        ON tst1.id = tst2.id;
