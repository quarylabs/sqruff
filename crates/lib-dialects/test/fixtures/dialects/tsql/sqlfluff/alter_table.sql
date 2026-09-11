ALTER TABLE dbo.Products ADD RetailValue AS [QtyAvailable] * UnitPrice * 1.5 PERSISTED; GO
ALTER TABLE dbo.Products ADD RetailValue AS (QtyAvailable * [UnitPrice] * 1.5) PERSISTED NOT NULL; GO
ALTER TABLE dbo.Products ADD InventoyDate AS CAST([InventoryTs] AS date); GO

ALTER TABLE [HangFire].[JobParameter]
ADD CONSTRAINT [FK_HangFire_JobParameter_Job]
FOREIGN KEY ([JobId])
REFERENCES [HangFire].[Job] ([Id])
ON UPDATE CASCADE
ON DELETE CASCADE; GO

ALTER TABLE [TestTable] DROP PERIOD FOR SYSTEM_TIME;
ALTER TABLE [TestTable] ADD PERIOD FOR SYSTEM_TIME (StartDate, EndDate);
