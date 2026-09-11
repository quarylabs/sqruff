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
