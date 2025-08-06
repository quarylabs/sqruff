# Flatten NodeMatcher Progress

## Objective
Remove unnecessary SyntaxKind entries that were added for NodeMatcher and flatten them into direct matchers.

## SyntaxKind Additions Reviewed and Flattened

### ✅ Successfully Flattened (NodeMatcher instances):
1. [x] AlterMasterKeyStatement
2. [x] AlterSecurityPolicyStatement
3. [x] AlterTableSwitchStatement
4. [x] BeginEndBlock
5. [x] CreateDatabaseScopedCredentialStatement
6. [x] CreateExternalDataSourceStatement
7. [x] CreateExternalFileFormatStatement
8. [x] CreateLoginStatement
9. [x] CreateMasterKeyStatement
10. [x] CreateSecurityPolicyStatement
11. [x] CreateSynonymStatement
12. [x] DeclareCursorStatement
13. [x] DropMasterKeyStatement
14. [x] DropSecurityPolicyStatement
15. [x] DropSynonymStatement
16. [x] ElseIfStatement
17. [x] ElseStatement
18. [x] JsonNullClause
19. [x] OffsetClause
20. [x] ReconfigureStatement
21. [x] RenameObjectStatement
22. [x] SetContextInfoStatement
23. [x] TryCatchStatement

### ⚠️ TypedParser instances (not NodeMatcher - no action needed):
1. AdditionAssignmentSegment - Uses TypedParser
2. DivisionAssignmentSegment - Uses TypedParser
3. ModulusAssignmentSegment - Uses TypedParser
4. MultiplicationAssignmentSegment - Uses TypedParser
5. SubtractionAssignmentSegment - Uses TypedParser

### ✅ Additional Entries Processed:
24. [x] PivotExpression - Flattened the NodeMatcher for PivotUnpivotStatementSegment
25. [x] PivotColumnReference - Kept as NodeMatcher per user request
26. [x] SelectIntoClause - Removed unused SyntaxKind entry
27. [x] UnpivotExpression - Removed unused SyntaxKind entry  
28. [x] OpenCursorStatement - Removed unused SyntaxKind entry
29. [x] DeallocateCursorStatement - Removed unused SyntaxKind entry

## Summary

All NodeMatcher instances have been successfully flattened and the corresponding SyntaxKind entries removed from syntax.rs. The TypedParser instances don't need flattening as they are already in the correct form. Tests have been updated and are passing.

## Commits Created
- First batch: "fix: Flatten NodeMatcher instances in T-SQL dialect (part 1)"
- Second batch: "fix: Complete NodeMatcher flattening for T-SQL dialect"