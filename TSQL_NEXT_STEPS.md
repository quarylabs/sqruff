# T-SQL Parsing - Prioritized Next Steps

## Immediate High-Impact Fixes (Start Here)

### 1. CREATE INDEX with T-SQL Extensions
**Impact**: Fixes 4 test files  
**Complexity**: Medium  
**Why First**: Indexes are fundamental DDL that many scripts use

Key features to add:
- CLUSTERED/NONCLUSTERED keywords
- INCLUDE (columns) clause  
- WHERE clause for filtered indexes
- WITH options (FILLFACTOR, ONLINE, PAD_INDEX, etc.)

### 2. WAITFOR Statements
**Impact**: Fixes 1 test file  
**Complexity**: Low  
**Why Early**: Simple to implement, common in stored procedures

Just two variants:
- `WAITFOR TIME 'HH:MM:SS'`
- `WAITFOR DELAY 'HH:MM:SS'`

### 3. CREATE TYPE Statements  
**Impact**: Fixes 1 test file  
**Complexity**: Low-Medium  
**Why Early**: Building block for table-valued parameters

Two variants:
- `CREATE TYPE name AS TABLE (...)` 
- `CREATE TYPE name FROM base_type`

### 4. OFFSET...FETCH Clause
**Impact**: Fixes 1 test file  
**Complexity**: Low  
**Why Early**: Modern pagination syntax, easy to add to SELECT

### 5. Statistics Operations
**Impact**: Part of add_index.sql  
**Complexity**: Medium  
**Why Early**: Often used with index operations

## Medium Priority (Good Next Batch)

### 6. BULK INSERT
**Impact**: Fixes 1 test file  
**Complexity**: Medium  
**Why**: Common data loading operation

### 7. ALTER INDEX  
**Impact**: Fixes 1 test file  
**Complexity**: Low-Medium  
**Why**: Natural companion to CREATE INDEX

### 8. TRY...CATCH Blocks
**Impact**: Fixes 1 test file  
**Complexity**: Medium  
**Why**: Error handling is important for robust scripts

### 9. Arithmetic Assignment Operators
**Impact**: Fixes 1 test file  
**Complexity**: Low  
**Why**: Common in UPDATE statements (`+=`, `-=`, etc.)

### 10. UNPIVOT Clause
**Impact**: Fixes 1 test file  
**Complexity**: Medium  
**Why**: Companion to already-implemented PIVOT

## Lower Priority (Can Wait)

- Partition functions/schemes (specialized feature)
- External objects (Azure/Polybase specific)  
- Security objects (less common in general scripts)
- SQLCMD commands (tooling specific)
- TABLESAMPLE (rarely used)
- Synonyms (less common)

## Recommended Implementation Order

1. Start with CREATE INDEX - biggest impact
2. Add WAITFOR and CREATE TYPE - quick wins
3. Add OFFSET...FETCH - completes SELECT syntax
4. Continue with statistics operations
5. Then tackle the medium priority items

This order balances impact, complexity, and logical grouping of related features.