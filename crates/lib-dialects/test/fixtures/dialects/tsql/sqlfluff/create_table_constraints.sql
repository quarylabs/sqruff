CREATE TABLE table_name (
    id UNIQUEIDENTIFIER NOT NULL
    CONSTRAINT constraint_name
    REFERENCES referenced_table_name
    ON DELETE NO ACTION
);
