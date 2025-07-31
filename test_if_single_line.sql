IF 1 <= (SELECT Weight from DimProduct WHERE ProductKey = 1)
    SELECT ProductKey FROM DimProduct WHERE ProductKey = 1
ELSE
    SELECT ProductKey FROM DimProduct WHERE ProductKey = 1