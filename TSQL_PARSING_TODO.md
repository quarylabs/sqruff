# T-SQL Parsing TODO - Path to 100% Parseability

## Current Status
- 68 out of 153 T-SQL test files (44%) contain unparseable sections
- 6 issues already fixed (#1793, #1794, #1806, #1807, #1808, #1809)

## Implementation Strategy

### Phase 1: Core Statement Types (High Priority)

#### 1. WAITFOR Statements
- **Files**: waitfor.sql
- **Syntax**: `WAITFOR TIME 'HH:MM:SS'` | `WAITFOR DELAY 'HH:MM:SS'`
- **Implementation**: Add WaitforStatementSegment with TIME/DELAY options

#### 2. CREATE TYPE Statements
- **Files**: create_type.sql
- **Syntax**: 
  - `CREATE TYPE name AS TABLE (column_definitions)`
  - `CREATE TYPE name FROM base_type`
- **Implementation**: Add CreateTypeStatementSegment with AS TABLE and FROM variants

#### 3. BULK INSERT Statement
- **Files**: bulk_insert.sql
- **Syntax**: `BULK INSERT table FROM 'file' WITH (options)`
- **Implementation**: Add BulkInsertStatementSegment with WITH clause options

#### 4. Partition Management
- **Files**: create_partition_function.sql, create_partition_scheme.sql
- **Syntax**:
  - `CREATE PARTITION FUNCTION name (type) AS RANGE LEFT/RIGHT FOR VALUES (...)`
  - `ALTER PARTITION FUNCTION name() SPLIT/MERGE RANGE (...)`
  - `CREATE PARTITION SCHEME name AS PARTITION function_name TO (...)`
- **Implementation**: Add partition-specific statement segments

### Phase 2: Index Operations (High Priority)

#### 5. CREATE INDEX with T-SQL Extensions
- **Files**: add_index.sql, create_columnstore_index.sql, create_fulltext_index.sql
- **Issues**:
  - CLUSTERED/NONCLUSTERED keywords
  - INCLUDE clause
  - WHERE clause
  - WITH options (FILLFACTOR, PAD_INDEX, ONLINE, etc.)
  - ON filegroup
- **Implementation**: Override CreateIndexStatementSegment with T-SQL specific grammar

#### 6. ALTER INDEX Statements
- **Files**: alter_index.sql
- **Syntax**: `ALTER INDEX name ON table REBUILD/REORGANIZE/DISABLE`
- **Implementation**: Add AlterIndexStatementSegment

#### 7. Statistics Management
- **Files**: add_index.sql (contains CREATE/UPDATE/DROP STATISTICS)
- **Syntax**:
  - `CREATE STATISTICS name ON table (columns)`
  - `UPDATE STATISTICS table (statistics_name) WITH options`
  - `DROP STATISTICS table.statistics_name`
- **Implementation**: Add statistics-specific statement segments

### Phase 3: Advanced DDL (Medium Priority)

#### 8. ALTER TABLE ... SWITCH
- **Files**: alter_table_switch.sql
- **Syntax**: `ALTER TABLE source SWITCH PARTITION n TO target`
- **Implementation**: Extend ALTER TABLE grammar with SWITCH clause

#### 9. External Objects
- **Files**: create_external_*.sql, drop_external_table.sql
- **Syntax**:
  - `CREATE EXTERNAL TABLE`
  - `CREATE EXTERNAL DATA SOURCE`
  - `CREATE EXTERNAL FILE FORMAT`
- **Implementation**: Add external object statement segments

#### 10. Security Objects
- **Files**: create_login.sql, create_user.sql, create_security_policy.sql
- **Implementation**: Add security-specific statement segments

### Phase 4: Procedural Elements (Medium Priority)

#### 11. Error Handling
- **Files**: raiserror.sql, try_catch.sql
- **Syntax**:
  - `RAISERROR (message, severity, state)`
  - `BEGIN TRY ... END TRY BEGIN CATCH ... END CATCH`
- **Implementation**: Add error handling statement segments

#### 12. Cursor Operations
- **Files**: cursor.sql
- **Syntax**: `DECLARE cursor_name CURSOR FOR select_statement`
- **Implementation**: Add cursor-specific statement segments

#### 13. CREATE OR ALTER Syntax
- **Files**: create_view_with_pivot.sql, minimal_function.sql
- **Syntax**: `CREATE OR ALTER FUNCTION/PROCEDURE/VIEW`
- **Implementation**: Extend CREATE statements with OR ALTER option

### Phase 5: Query Extensions (Medium Priority)

#### 14. OFFSET...FETCH
- **Files**: offset.sql
- **Syntax**: `OFFSET n ROWS FETCH NEXT m ROWS ONLY`
- **Implementation**: Add to SELECT statement grammar

#### 15. UNPIVOT Clause
- **Files**: create_view_with_unpivot.sql
- **Syntax**: Similar to PIVOT but opposite operation
- **Implementation**: Add UnpivotClauseSegment

#### 16. TABLESAMPLE Clause
- **Files**: tablesample.sql
- **Syntax**: `TABLESAMPLE SYSTEM (n PERCENT)`
- **Implementation**: Add to FROM clause grammar

### Phase 6: Minor Features (Low Priority)

#### 17. Miscellaneous Statements
- RECONFIGURE statements
- RENAME OBJECT statements
- CREATE SYNONYM statements
- SET IDENTITY_INSERT ON/OFF
- SET CONTEXT_INFO
- Arithmetic assignment operators (+=, -=, etc.)
- GRANT/DENY/REVOKE permissions
- SQLCMD commands
- COPY statement

## Testing Strategy

1. Fix each category of statements
2. Run `UPDATE_EXPECT=1 cargo test -p sqruff-lib-dialects --test dialects tsql`
3. Verify YAML files are updated correctly
4. Ensure no regressions in previously passing tests
5. Final verification: all 153 T-SQL test files should parse without unparseable sections

## Success Metrics

- Zero "unparsable:" entries in all T-SQL YAML files
- All dialect tests pass
- Full compatibility with SQLFluff T-SQL test suite