-- Basic table variable usage
SELECT [value] FROM @DepartmentCodes;

-- Table variable with alias
SELECT ids.[value] FROM @DepartmentCodes AS ids;

-- Table variable with WHERE clause
SELECT * FROM @TableVariable WHERE [value] > 0;

-- Table variable with table hints
SELECT * FROM @TableVariable WITH (NOLOCK);

-- Table variable in subquery
SELECT * FROM table1 WHERE id IN (SELECT [value] FROM @DepartmentCodes);