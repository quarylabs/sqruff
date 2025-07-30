-- Basic single column
EXECUTE test WITH RESULT SETS ((col1 INT));

-- Multiple columns
EXECUTE test WITH RESULT SETS ((col1 INT, col2 NVARCHAR(50), col3 DECIMAL(10,2)));

-- With NULL constraints
EXECUTE test WITH RESULT SETS ((col1 INT NULL, col2 VARCHAR(100) NOT NULL));

-- Multiple result sets
EXECUTE test WITH RESULT SETS (
    (col1 INT, col2 VARCHAR(50)),
    (name NVARCHAR(100), count INT)
);

-- With bracketed identifiers
EXECUTE test WITH RESULT SETS (([Column 1] INT, [Column 2] NVARCHAR(MAX)));

-- NONE and UNDEFINED
EXECUTE test WITH RESULT SETS NONE;
EXECUTE test WITH RESULT SETS UNDEFINED;