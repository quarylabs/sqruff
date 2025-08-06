# Flatten NodeMatcher Progress

## Objective
Remove unnecessary SyntaxKind entries that were added for NodeMatcher and flatten them into direct matchers.

## SyntaxKind Additions to Review and Flatten

The following SyntaxKind entries were added in this PR and need to be reviewed:

1. [ ] AdditionAssignmentSegment
2. [ ] AlterMasterKeyStatement
3. [ ] AlterSecurityPolicyStatement
4. [ ] AlterTableSwitchStatement
5. [ ] BeginEndBlock
6. [ ] CreateDatabaseScopedCredentialStatement
7. [ ] CreateExternalDataSourceStatement
8. [ ] CreateExternalFileFormatStatement
9. [ ] CreateLoginStatement
10. [ ] CreateMasterKeyStatement
11. [ ] CreateSecurityPolicyStatement
12. [ ] CreateSynonymStatement
13. [ ] DeallocateCursorStatement
14. [ ] DeclareCursorStatement
15. [ ] DivisionAssignmentSegment
16. [ ] DropMasterKeyStatement
17. [ ] DropSecurityPolicyStatement
18. [ ] DropSynonymStatement
19. [ ] ElseIfStatement
20. [ ] ElseStatement
21. [ ] JsonNullClause
22. [ ] ModulusAssignmentSegment
23. [ ] MultiplicationAssignmentSegment
24. [ ] OffsetClause
25. [ ] OpenCursorStatement
26. [ ] PivotColumnReference
27. [ ] PivotExpression
28. [ ] ReconfigureStatement
29. [ ] RenameObjectStatement
30. [ ] SelectIntoClause
31. [ ] SetContextInfoStatement
32. [ ] SubtractionAssignmentSegment
33. [ ] TryCatchStatement
34. [ ] UnpivotExpression

## Progress Notes

### Step 1: Identify NodeMatcher Usage
For each SyntaxKind, find where it's used with NodeMatcher in the dialect files and convert to flat matcher.

### Step 2: Remove SyntaxKind Entry
After flattening the matcher, remove the SyntaxKind entry from syntax.rs.

### Step 3: Update Test Expectations
Run tests with UPDATE_EXPECT=1 to update any affected test files.

### Step 4: Verify Tests Pass
Ensure all tests pass after the changes.

## Current Status
Starting review...