CREATE TABLE [dbo].[example](
    [Column A] [int] IDENTITY(1,1),
    [Column B] [int] IDENTITY(1, 1) NOT NULL,
    [ColumnC] varchar(100) DEFAULT 'mydefault',
    [ColumnDecimal] DATE, -- DEFAULT GETDATE() -- Not implemented yet
    [ColumnUser] char(30), -- DEFAULT CURRENT_USER -- Not implemented yet
    [col1] int default -1 not null,
    [col2] int default -1 not null,
    [col3] int default -1 not null,
    [col4] INT DEFAULT NULL
)
GO

create table [schema1].[table1] (
	[col1] INT,
	PRIMARY KEY CLUSTERED ([col1] ASC)
)
GO

create table [schema1].[table1] (
	[col1] INT,
	CONSTRAINT [Pk_Id] PRIMARY KEY NONCLUSTERED ([col1] DESC)
)
GO

CREATE TABLE [dbo].[table1] (
    [ColumnB] [varchar](100), -- FILESTREAM MASKED WITH (FUNCTION = 'my_func') -- Not implemented yet
    [ColumnC] varchar(100) NULL, -- NOT FOR REPLICATION -- Not implemented yet
    [ColumnDecimal] decimal(10,3), -- GENERATED ALWAYS AS ROW START HIDDEN -- Not implemented yet
    [columnE] varchar(100), -- ENCRYPTED WITH (...) -- Not implemented yet
    [column1] varchar (100) collate Latin1_General_BIN
)
GO

CREATE TABLE table_name (
    id UNIQUEIDENTIFIER NOT NULL,
    CONSTRAINT constraint_name FOREIGN KEY (id) REFERENCES referenced_table_name (id) ON DELETE NO ACTION
);
