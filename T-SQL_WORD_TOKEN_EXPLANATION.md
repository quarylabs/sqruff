# T-SQL Word Token Behavior Explained

## Understanding the Issue

When examining `stored_procedure_begin_end.yml`, you'll notice that all SQL keywords (CREATE, PROCEDURE, SELECT, BEGIN, END, etc.) are lexed as `word` tokens instead of proper keyword tokens. This is **by design** in T-SQL's context-dependent lexing.

## Why This Happens

T-SQL has a unique lexing behavior where keywords can be used as identifiers in certain contexts. To support this, the lexer treats most keywords as regular words when inside:
- Stored procedure bodies
- Function bodies  
- Trigger definitions
- Other procedural code contexts

## The Solution: Word-Aware Parsers

To handle this, sqruff implements "word-aware" parsers that can recognize SQL statement patterns even when the tokens are lexed as words:

### 1. Token Stream (What You See in YAML)
```yaml
- word: CREATE
- word: PROCEDURE
- word: dbo
- dot: .
- word: Test_Begin_End
- word: AS
- word: BEGIN
- word: SELECT
```

### 2. Parsed Structure (What the Parser Creates)
Despite being word tokens, these are parsed into proper AST nodes:
- `CreateProcedureStatement`
- `BeginEndBlock`
- `SelectStatement`

### 3. Word-Aware Parsers Created
- `WordAwareTryCatchSegment` - Parses TRY/CATCH blocks
- `WordAwareCreateIndexStatementSegment` - Parses CREATE INDEX
- `WordAwareCreateTriggerStatementSegment` - Parses CREATE TRIGGER
- `WordAwareCreateProcedureSegment` - Parses CREATE PROCEDURE
- `WordAwareBeginEndBlockSegment` - Parses BEGIN/END blocks
- `WordAwareIfStatementSegment` - Parses IF statements
- `WordAwareSelectStatementSegment` - Parses SELECT statements
- And many more...

## Key Implementation Details

### 1. StringParser with SyntaxKind::Word
Word-aware parsers use `StringParser::new("KEYWORD", SyntaxKind::Word)` to match keywords that have been lexed as words:

```rust
StringParser::new("CREATE", SyntaxKind::Word),
StringParser::new("PROCEDURE", SyntaxKind::Word),
```

### 2. Parser Ordering
Word-aware parsers must be ordered correctly in `WordAwareStatementSegment` to ensure proper precedence and avoid conflicts.

### 3. Terminators
Each parser includes appropriate terminators to prevent over-consumption of tokens:

```rust
this.terminators = vec_of_erased![
    StringParser::new("GO", SyntaxKind::Word),
    StringParser::new("CREATE", SyntaxKind::Word),
    // etc...
];
```

## Limitations

While word-aware parsers allow successful parsing, they cannot create as detailed AST structures as when keywords are properly lexed. This is a fundamental limitation of T-SQL's context-dependent lexing that would require significant architectural changes to fully resolve.

## Testing

The YAML test files show the raw token stream, not the parsed result. Therefore:
- Seeing `word` tokens in YAML files is **expected** and **correct**
- The success metric is whether these word tokens can be parsed without errors
- Linting rules can still operate on the parsed structures

## Conclusion

The presence of word tokens in T-SQL test fixtures is not a bug - it's the expected behavior of T-SQL's lexer. The word-aware parsing system successfully handles these tokens to create functional AST structures that enable linting and formatting operations.