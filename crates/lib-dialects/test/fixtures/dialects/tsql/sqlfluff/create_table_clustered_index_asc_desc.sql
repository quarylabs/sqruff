CREATE TABLE [dbo].[table] ( [name] [varchar](100) NOT NULL, [month_num] [int] NULL )
WITH ( DISTRIBUTION = REPLICATE, CLUSTERED INDEX ( [name] ASC, [month_num] ASC ) )
GO

CREATE TABLE [dbo].[table2] ( [name] [varchar](100) NOT NULL, [month_num] [int] NULL )
WITH ( DISTRIBUTION = REPLICATE, CLUSTERED INDEX ( [name] DESC, [month_num] ) )
GO
