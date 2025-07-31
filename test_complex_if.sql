IF 1 <= (SELECT Weight from DimProduct WHERE ProductKey = 1)
    SELECT ProductKey, EnglishDescription, Weight
    FROM DimProduct WHERE ProductKey = 1
ELSE
    SELECT ProductKey, EnglishDescription, Weight  
    FROM DimProduct WHERE ProductKey = 1;