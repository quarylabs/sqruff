-- Test T-SQL data types with MAX and -1 parameters
-- MAX is equivalent to the maximum storage size
-- -1 is equivalent to MAX for certain data types

-- Variable declarations with MAX
DECLARE @LargeText NVARCHAR(MAX);
DECLARE @BigString VARCHAR(MAX);
DECLARE @BinaryData VARBINARY(MAX);

-- Variable declarations with -1 (equivalent to MAX)
DECLARE @LargeText2 NVARCHAR(-1);
DECLARE @BigString2 VARCHAR(-1);
DECLARE @BinaryData2 VARBINARY(-1);