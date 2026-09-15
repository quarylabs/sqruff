select
    pvt.cl1
    , pvt.cl2
    , pvt.cl3
    , [1] as lvl_1
    , [2] as lvl_2
    , [3] as lvl_3
from
    levels as lvl
pivot
(max(value) for rn in([1], [2], [3]) ) as pvt;

SELECT pvt.cl1 FROM levels AS lvl
PIVOT (MAX(value) FOR rn IN ([1], [2], [3])) AS [pvt];

SELECT unpvt.amount FROM monthly AS m
UNPIVOT (amount FOR month_name IN ([Jan], [Feb], [Mar])) AS unpvt;
