CREATE TABLE table_name (
    id UNIQUEIDENTIFIER NOT NULL
    CONSTRAINT constraint_name
    REFERENCES referenced_table_name
    ON DELETE NO ACTION
);

CREATE TABLE dbo.SomeTable (ID int not null IDENTITY(-2147483648, 1) PRIMARY KEY);
