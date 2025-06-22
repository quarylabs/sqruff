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

-- Table creation with MAX columns
CREATE TABLE TestMaxTypes (
    ID INT IDENTITY(1,1) PRIMARY KEY,
    LargeText NVARCHAR(MAX),
    BigString VARCHAR(MAX),
    XmlData XML,
    BinaryData VARBINARY(MAX),
    -- With NOT NULL constraints
    RequiredText NVARCHAR(MAX) NOT NULL,
    -- With default values
    DefaultText VARCHAR(MAX) DEFAULT 'Default Value'
);

-- Table creation with -1 columns
CREATE TABLE TestNegativeOneTypes (
    ID INT IDENTITY(1,1) PRIMARY KEY,
    LargeText NVARCHAR(-1),
    BigString VARCHAR(-1),
    BinaryData VARBINARY(-1),
    -- Mixed with regular sized columns
    FixedText NVARCHAR(100),
    SmallString VARCHAR(50)
);

-- CAST operations with MAX
SELECT 
    CAST(column1 AS NVARCHAR(MAX)) AS TextMax,
    CAST(column2 AS VARCHAR(MAX)) AS StringMax,
    CAST(column3 AS VARBINARY(MAX)) AS BinaryMax
FROM SourceTable;

-- CAST operations with -1
SELECT 
    CAST(column1 AS NVARCHAR(-1)) AS TextNegOne,
    CAST(column2 AS VARCHAR(-1)) AS StringNegOne,
    CAST(column3 AS VARBINARY(-1)) AS BinaryNegOne
FROM SourceTable;

-- CONVERT operations
SELECT 
    CONVERT(NVARCHAR(MAX), column1) AS ConvertedMax,
    CONVERT(VARCHAR(-1), column2) AS ConvertedNegOne
FROM SourceTable;

-- Function with MAX parameters
CREATE FUNCTION GetLongText(@ID INT)
RETURNS NVARCHAR(MAX)
AS
BEGIN
    DECLARE @Result NVARCHAR(MAX);
    SELECT @Result = LongTextColumn FROM TextTable WHERE ID = @ID;
    RETURN @Result;
END;

-- Stored procedure with MAX and -1 parameters
CREATE PROCEDURE ProcessLargeData
    @InputText NVARCHAR(MAX),
    @OutputText NVARCHAR(-1) OUTPUT
AS
BEGIN
    SET @OutputText = UPPER(@InputText);
END;

-- Using MAX in temporary tables
CREATE TABLE #TempMax (
    TempID INT,
    TempText NVARCHAR(MAX),
    TempBinary VARBINARY(-1)
);

-- Table-valued parameters with MAX
CREATE TYPE LargeTextTable AS TABLE (
    TextID INT,
    TextValue NVARCHAR(MAX)
);

-- Complex data type combinations
DECLARE @MixedTypes TABLE (
    SmallText NVARCHAR(10),
    MediumText NVARCHAR(4000),
    LargeText NVARCHAR(MAX),
    HugeText NVARCHAR(-1),
    RegularInt INT,
    BigBinary VARBINARY(MAX)
);