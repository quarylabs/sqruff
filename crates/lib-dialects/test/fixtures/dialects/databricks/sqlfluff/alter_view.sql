-- ALTER TABLE examples from Databricks documentation
-- https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-alter-view.html

ALTER VIEW tempsc1.v1 RENAME TO tempsc1.v2;

ALTER VIEW IDENTIFIER('tempsc1.v1') RENAME TO IDENTIFIER('tempsc1.v2');

