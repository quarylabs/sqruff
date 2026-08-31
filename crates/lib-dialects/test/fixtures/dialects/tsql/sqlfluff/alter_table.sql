ALTER TABLE dbo.Products ADD RetailValue AS [QtyAvailable] * UnitPrice * 1.5 PERSISTED; GO
ALTER TABLE dbo.Products ADD RetailValue AS (QtyAvailable * [UnitPrice] * 1.5) PERSISTED NOT NULL; GO
ALTER TABLE dbo.Products ADD InventoyDate AS CAST([InventoryTs] AS date); GO
