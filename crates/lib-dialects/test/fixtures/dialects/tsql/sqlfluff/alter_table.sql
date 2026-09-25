ALTER TABLE dbo.Products ADD RetailValue AS [QtyAvailable] * UnitPrice * 1.5 PERSISTED; GO
ALTER TABLE dbo.Products ADD RetailValue AS (QtyAvailable * [UnitPrice] * 1.5) PERSISTED NOT NULL; GO
ALTER TABLE dbo.Products ADD InventoyDate AS CAST([InventoryTs] AS date); GO

ALTER TABLE [HangFire].[JobParameter]
ADD CONSTRAINT [FK_HangFire_JobParameter_Job]
FOREIGN KEY ([JobId])
REFERENCES [HangFire].[Job] ([Id])
ON UPDATE CASCADE
ON DELETE CASCADE; GO

-- Drop multiple columns in one statement
ALTER TABLE UserData DROP COLUMN [StrSkill], [StrItem], [StrSerial];
ALTER TABLE UserData DROP COLUMN IF EXISTS StrSkill, StrItem, StrSerial;

-- Check hexadecimal defaults in constraints
CREATE TABLE [dbo].[UserData] (
    [strUserId] [char](21) NOT NULL,
    [strItem] [binary](400) NULL,
    [strSkill] [binary](400) NULL,
    CONSTRAINT PK_UserData PRIMARY KEY CLUSTERED ([strUserId] ASC)
);

ALTER TABLE [dbo].[UserData]
ADD CONSTRAINT [DF_UserData_strSkill] DEFAULT (0x00) FOR [strSkill];
GO

ALTER TABLE [TestTable] DROP PERIOD FOR SYSTEM_TIME;
ALTER TABLE [TestTable] ADD PERIOD FOR SYSTEM_TIME (StartDate, EndDate);
ALTER TABLE [TestTable] ADD
StartDate DATETIME2,
EndDate DATETIME2,
PERIOD FOR SYSTEM_TIME (StartDate, EndDate);

ALTER TABLE Production.TransactionHistoryArchive
DROP CONSTRAINT PK_TransactionHistoryArchive_TransactionID;

ALTER TABLE Production.TransactionHistoryArchive
DROP CONSTRAINT IF EXISTS PK_TransactionHistoryArchive_TransactionID;

ALTER TABLE Production.Transactionhistoryarchive
DROP Pk_transactionhistoryarchive_transactionid;

ALTER TABLE Production.TransactionHistoryArchive
CHECK CONSTRAINT PK_TransactionHistoryArchive_TransactionID;

ALTER TABLE [Production].[ProductCostHistory]
CHECK CONSTRAINT [FK_ProductCostHistory_Product_ProductID]

ALTER TABLE Purchasing.PurchaseOrderHeader
NOCHECK CONSTRAINT FK_PurchaseOrderHeader_Employee_EmployeeID;

ALTER TABLE [dbo].[Attachment]
WITH CHECK
CHECK CONSTRAINT [FK_Attachment_EmailMessage];

ALTER TABLE [dbo].[Attachment]
WITH CHECK
NOCHECK CONSTRAINT [FK_Attachment_EmailMessage];

ALTER TABLE [dbo].[Attachment]
WITH NOCHECK
NOCHECK CONSTRAINT [FK_Attachment_EmailMessage];

ALTER TABLE [dbo].[Attachment]
WITH NOCHECK
CHECK CONSTRAINT [FK_Attachment_EmailMessage];

ALTER TABLE [TestTable] REBUILD;
ALTER TABLE [TestTable] REBUILD PARTITION=ALL;
ALTER TABLE [TestTable] REBUILD PARTITION=1;
ALTER TABLE [TestTable] REBUILD WITH (DATA_COMPRESSION=PAGE, XML_COMPRESSION=ON);
ALTER TABLE [TestTable] REBUILD PARTITION=1 WITH (DATA_COMPRESSION=ROW);
ALTER TABLE [TestTable] REBUILD PARTITION=ALL WITH (
  XML_COMPRESSION = ON,
  DATA_COMPRESSION = NONE ON PARTITIONS (4),
  DATA_COMPRESSION = COLUMNSTORE ON PARTITIONS (1, 5 TO 7, 10, 20 TO 40)
  );

ALTER TABLE dbo.SomeTable DROP
  CONSTRAINT IF EXISTS SomeConstraint,
  CONSTRAINT SomeOtherConstraint;

ALTER TABLE dbo.SomeTable DROP
  COLUMN SomeColumn,
  COLUMN IF EXISTS SomeOtherColumn;
