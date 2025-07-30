-- Test case that might have parsing issues
EXECUTE test WITH RESULT SETS (
    (
        col1 INT,
        col2 VARCHAR(50) NOT NULL,
        col3 DECIMAL(10,2)
    )
);