IF 1 <= (SELECT Weight from DimProduct WHERE ProductKey = 1)
    SELECT ProductKey, EnglishDescription
    FROM DimProduct WHERE ProductKey = 1
ELSE
    SELECT ProductKey, EnglishDescription
    FROM DimProduct WHERE ProductKey = 1