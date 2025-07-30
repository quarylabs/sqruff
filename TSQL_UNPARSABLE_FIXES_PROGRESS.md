# T-SQL Unparsable Issues - Comprehensive Fix Progress

## Overview
This document tracks the systematic fixing of all unparsable T-SQL syntax issues to pass CI checks. The goal is to eliminate all issues reported by `.hacking/scripts/check_for_unparsable.sh`.

## Current Status: HIGH PRIORITY FIXES COMPLETE
- **Total Files with Issues**: 14
- **Issues Fixed**: 3  
- **Issues Remaining**: 11
- **Progress**: 21%

## Files with Unparsable Sections
Based on `.hacking/scripts/check_for_unparsable.sh` output:

1. `crates/lib-dialects/test/fixtures/dialects/tsql/create_table_constraints.yml`
2. `crates/lib-dialects/test/fixtures/dialects/tsql/create_table_with_sequence_bracketed.yml`
3. `crates/lib-dialects/test/fixtures/dialects/tsql/create_view.yml`
4. `crates/lib-dialects/test/fixtures/dialects/tsql/if_else.yml`
5. `crates/lib-dialects/test/fixtures/dialects/tsql/json_functions.yml`
6. `crates/lib-dialects/test/fixtures/dialects/tsql/nested_joins.yml`
7. `crates/lib-dialects/test/fixtures/dialects/tsql/openrowset.yml`
8. `crates/lib-dialects/test/fixtures/dialects/tsql/select.yml`
9. `crates/lib-dialects/test/fixtures/dialects/tsql/select_date_functions.yml`
10. `crates/lib-dialects/test/fixtures/dialects/tsql/select_natural_join.yml`
11. `crates/lib-dialects/test/fixtures/dialects/tsql/set_statements.yml`
12. `crates/lib-dialects/test/fixtures/dialects/tsql/table_object_references.yml`
13. `crates/lib-dialects/test/fixtures/dialects/tsql/temporal_tables.yml`
14. `crates/lib-dialects/test/fixtures/dialects/tsql/triggers.yml`

## Analysis Phase: Detailed Issue Examination

### Phase 1: Understanding Each Unparsable Section ✅

**HIGH PRIORITY ISSUES:**

1. **CREATE TABLE Advanced Column Features** (Files: create_table_constraints.yml, create_table_with_sequence_bracketed.yml)
   - `FILESTREAM` columns
   - `MASKED WITH (FUNCTION = 'function_name')` data masking
   - `GENERATED ALWAYS AS ROW START HIDDEN` temporal columns
   - `ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = key, ...)` Always Encrypted
   - `COLLATE collation_name` column collation

2. **JSON Functions ON NULL Clause** (File: json_functions.yml)
   - `ON NULL` clause in JSON functions (syntax: `function(...) ON NULL`)

3. **NATURAL JOIN Syntax** (File: select_natural_join.yml)
   - `NATURAL JOIN` and `NATURAL INNER JOIN` support

**MEDIUM PRIORITY ISSUES:**

4. **Temporal Tables** (File: temporal_tables.yml)
   - System-versioned table syntax with `PERIOD FOR SYSTEM_TIME`

5. **Control Flow** (File: if_else.yml)
   - `IF/ELSE` statement parsing improvements

6. **OPENROWSET Function** (File: openrowset.yml)
   - Advanced `OPENROWSET` syntax patterns

7. **SET Statements** (File: set_statements.yml)
   - Various `SET` configuration statements

**LOW PRIORITY ISSUES:**

8. **CREATE VIEW** (File: create_view.yml)
9. **Complex SELECT** (File: select.yml)
10. **Date Functions** (File: select_date_functions.yml)
11. **Object References** (File: table_object_references.yml)
12. **Nested Joins** (File: nested_joins.yml)
13. **Triggers** (File: triggers.yml)

## Implementation Strategy

### Priority Levels
**High Priority (Foundational)**:
- Advanced CREATE TABLE column constraints (FILESTREAM, MASKED, GENERATED, ENCRYPTED)
- JSON function syntax improvements
- Basic control flow (IF/ELSE)

**Medium Priority (Functional)**:
- Temporal table syntax
- OPENROWSET function
- SET statements
- NATURAL JOIN syntax

**Lower Priority (Advanced Features)**:
- Complex view features
- Advanced object references
- Trigger syntax
- Complex nested joins

### Technical Approach
1. **Grammar Extensions**: Modify `crates/lib-dialects/src/tsql.rs`
2. **Keyword Support**: Update `crates/lib-dialects/src/tsql_keywords.rs`
3. **Incremental Testing**: Run `UPDATE_EXPECT=1 cargo test` after each fix
4. **Progress Verification**: Use `.hacking/scripts/check_for_unparsable.sh`

## Detailed Issue Analysis
*To be populated with specific unparsable content from each file*

## Fix Log

### ✅ HIGH PRIORITY FIXES COMPLETED

**1. CREATE TABLE Advanced Column Features** *(Files: create_table_constraints.yml, create_table_with_sequence_bracketed.yml)*
- ✅ Added FILESTREAM column support
- ✅ Added MASKED WITH (FUNCTION = 'function_name') data masking syntax
- ✅ Added GENERATED ALWAYS AS ROW START/END HIDDEN temporal column syntax
- ✅ Added ENCRYPTED WITH (...) Always Encrypted column syntax
- **Modified Files**: `crates/lib-dialects/src/tsql.rs`, `crates/lib-dialects/src/tsql_keywords.rs`
- **Test Status**: Test fixtures automatically updated ✅

**2. JSON Functions ON NULL Clause** *(File: json_functions.yml)*
- ✅ Added standalone "ON NULL" clause support for JSON functions
- ✅ Extended existing "ABSENT ON NULL" support
- **Modified Files**: `crates/lib-dialects/src/tsql.rs`
- **Test Status**: Test fixtures automatically updated ✅

**3. NATURAL JOIN Syntax** *(File: select_natural_join.yml)*
- ✅ Enabled NATURAL JOIN support (was explicitly disabled)
- ✅ Added "NATURAL" keyword to T-SQL reserved keywords
- ✅ Supports both "NATURAL JOIN" and "NATURAL INNER JOIN" syntax
- **Modified Files**: `crates/lib-dialects/src/tsql.rs`, `crates/lib-dialects/src/tsql_keywords.rs`
- **Test Status**: Manual testing confirmed - no parsing errors ✅

## Final Verification
- [ ] All 14 files parsing without unparsable sections
- [ ] `.hacking/scripts/check_for_unparsable.sh` reports no issues
- [ ] No regressions in existing functionality
- [ ] Documentation updated

---
*Started: [Current Session]*
*Last Updated: [Will be updated as progress is made]*