use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Nothing, Ref};
use sqruff_lib_core::parser::lexer::{Matcher, Pattern};
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::tsql_keywords::{FUTURE_RESERVED_KEYWORDS, RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    raw_dialect().config(|this| this.expand())
}

#[rustfmt::skip]
pub fn raw_dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let mut tsql_dialect = ansi_dialect.clone();
    tsql_dialect.name = DialectKind::Tsql;
    
    tsql_dialect.sets_mut("reserved_keywords").clear();
    
    tsql_dialect.sets_mut("unreserved_keywords").clear();
    
    tsql_dialect.sets_mut("future_reserved_keywords").clear();
    
    tsql_dialect.sets_mut("reserved_keywords").extend(RESERVED_KEYWORDS);
    
    tsql_dialect.sets_mut("unreserved_keywords").extend(UNRESERVED_KEYWORDS);
    
    tsql_dialect.sets_mut("future_reserved_keywords").extend(FUTURE_RESERVED_KEYWORDS);
    
    tsql_dialect.sets_mut("datetime_units").clear();
    
    tsql_dialect.sets_mut("datetime_units").extend(["D", "DAY", "DAYS", "DAYOFYEAR", "DD", "DW", "DY", "HH", "HOUR", "ISO_WEEK", "ISOWK", "ISOWW", "INFINITE", "M", "MCS", "MI", "MICROSECOND", "MILLISECOND", "MINUTE", "MM", "MONTH", "MONTHS", "MS", "N", "NANOSECOND", "NS", "Q", "QQ", "QUARTER", "S", "SECOND", "SS", "TZ", "TZOFFSET", "W", "WEEK", "WEEKS", "WEEKDAY", "WK", "WW", "YEAR", "YEARS", "Y", "YY", "YYYY"]);
    
    tsql_dialect.sets_mut("date_part_function_name").clear();
    
    tsql_dialect.sets_mut("date_part_function_name").extend(["DATEADD", "DATEDIFF", "DATEDIFF_BIG", "DATENAME", "DATEPART", "DATETRUNC"]);
    
    tsql_dialect.sets_mut("date_format").clear();
    
    tsql_dialect.sets_mut("date_format").extend(["mdy", "dmy", "ymd", "myd", "dym"]);
    
    tsql_dialect.sets_mut("bare_functions").extend(["CURRENT_USER", "SESSION_USER", "SYSTEM_USER", "USER"]);
    
    tsql_dialect.sets_mut("sqlcmd_operators").clear();
    
    tsql_dialect.sets_mut("sqlcmd_operators").extend(["r", "setvar"]);
    
    tsql_dialect.sets_mut("file_compression").clear();
    
    tsql_dialect.sets_mut("file_compression").extend(["'org.apache.hadoop.io.compress.GzipCodec'", "'org.apache.hadoop.io.compress.DefaultCodec'", "'org.apache.hadoop.io.compress.SnappyCodec'"]);
    
    tsql_dialect.sets_mut("file_encoding").clear();
    
    tsql_dialect.sets_mut("file_encoding").extend(["'UTF8'", "'UTF16'"]);
    
    tsql_dialect.sets_mut("serde_method").clear();
    
    tsql_dialect.sets_mut("serde_method").extend(["'org.apache.hadoop.hive.serde2.columnar.LazyBinaryColumnarSerDe'", "'org.apache.hadoop.hive.serde2.columnar.ColumnarSerDe'"]);
    
    tsql_dialect.insert_lexer_matchers(vec![
        Matcher::regex("atsign", r#"[@][a-zA-Z0-9_@$#]+"#, SyntaxKind::Atsign),
        Matcher::regex("square_quote", r#"\[([^\[\]]*)*\]"#, SyntaxKind::SquareQuote),
        Matcher::regex("single_quote_with_n", r#"N'([^']|'')*'"#, SyntaxKind::SingleQuoteWithN),
        Matcher::regex("hash_prefix", r#"[#][#]?[a-zA-Z0-9_@$#]+"#, SyntaxKind::HashPrefix),
        Matcher::legacy("unquoted_relative_sql_file_path", |_| true, r#"[.\w\\/#-]+\.[sS][qQ][lL]\b"#, SyntaxKind::UnquotedRelativeSqlFilePath),
    ], "back_quote");
    
    tsql_dialect.insert_lexer_matchers(vec![
        Matcher::regex("numeric_literal", r#"([xX]'([\da-fA-F][\da-fA-F])+'|0[xX][\da-fA-F]*)"#, SyntaxKind::NumericLiteral),
    ], "word");
    
    tsql_dialect.patch_lexer_matchers(vec![
        Matcher::regex("single_quote", r#"'([^']|'')*'"#, SyntaxKind::SingleQuote),
        Matcher::regex("inline_comment", r#"(--)[^\n]*"#, SyntaxKind::InlineComment),
        Matcher::native("block_comment", sqruff_lib_core::parser::lexer::nested_block_comment, SyntaxKind::BlockComment)
            .subdivider(Pattern::legacy("newline", |_| true, r#"\r\n|\n"#, SyntaxKind::Newline))
            .post_subdivide(Pattern::legacy("whitespace", |_| true, r#"[^\S\r\n]+"#, SyntaxKind::Whitespace)),
        Matcher::regex("word", r#"[0-9a-zA-Z_#@$\p{L}]+"#, SyntaxKind::Word),
    ]);
    
    tsql_dialect.add([
        (
            "PercentSegment".into(),
            TypedParser::new(SyntaxKind::Percent, SyntaxKind::Percent)
                .to_matchable()
                .into()
        ),
        (
            "BracketedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::SquareQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into()
        ),
        (
            "HashIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::HashPrefix, SyntaxKind::HashIdentifier)
                .to_matchable()
                .into()
        ),
        (
            "BatchDelimiterGrammar".into(),
            Ref::new("GoStatementSegment")
                .to_matchable()
                .into()
        ),
        (
            "QuotedLiteralSegmentWithN".into(),
            TypedParser::new(SyntaxKind::SingleQuoteWithN, SyntaxKind::QuotedLiteral)
                .to_matchable()
                .into()
        ),
        (
            "QuotedLiteralSegmentOptWithN".into(),
            one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentWithN") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "IntegerLiteralSegment".into(),
            RegexParser::new(r#"(?<!\.)\b\d+\b(?!\.\d)"#, SyntaxKind::IntegerLiteral)
                .to_matchable()
                .into()
        ),
        (
            "BinaryLiteralSegment".into(),
            RegexParser::new(r#"0[xX][\da-fA-F]*"#, SyntaxKind::BinaryLiteral)
                .to_matchable()
                .into()
        ),
        (
            "TransactionGrammar".into(),
            one_of(vec![Ref::keyword("TRANSACTION") .to_matchable(), Ref::keyword("TRAN") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "SystemVariableSegment".into(),
            RegexParser::new(r#"@@[A-Za-z0-9_]+"#, SyntaxKind::SystemVariable)
                .to_matchable()
                .into()
        ),
        (
            "StatementAndDelimiterGrammar".into(),
            one_of(vec![Sequence::new(vec![Ref::new("StatementSegment") .to_matchable(), Ref::new("DelimiterGrammar") .optional() .to_matchable()]) .to_matchable(), Ref::new("DelimiterGrammar") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "OneOrMoreStatementsGrammar".into(),
            AnyNumberOf::new(vec![Ref::new("StatementAndDelimiterGrammar") .to_matchable()])
                .config(|this| {
                    this.min_times(1);
                })
                .to_matchable()
                .into()
        ),
        (
            "TopPercentGrammar".into(),
            Sequence::new(vec![Ref::keyword("TOP") .to_matchable(), optionally_bracketed(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Ref::keyword("PERCENT") .optional() .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "CursorNameGrammar".into(),
            one_of(vec![Sequence::new(vec![Ref::keyword("GLOBAL") .optional() .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "CredentialGrammar".into(),
            Sequence::new(vec![Ref::keyword("IDENTITY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("SECRET") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "AzureBlobStoragePath".into(),
            RegexParser::new(r#"'https://[a-z0-9][a-z0-9-]{1,61}[a-z0-9]\.blob\.core\.windows\.net/[a-z0-9][a-z0-9\.-]{1,61}[a-z0-9](?:/.+)?'"#, SyntaxKind::ExternalLocation)
                .to_matchable()
                .into()
        ),
        (
            "AzureDataLakeStorageGen2Path".into(),
            RegexParser::new(r#"'https://[a-z0-9][a-z0-9-]{1,61}[a-z0-9]\.dfs\.core\.windows\.net/[a-z0-9][a-z0-9\.-]{1,61}[a-z0-9](?:/.+)?'"#, SyntaxKind::ExternalLocation)
                .to_matchable()
                .into()
        ),
        (
            "SqlcmdOperatorSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(dialect.sets("sqlcmd_operators").iter().map(|item| item.to_string()).collect_vec(), SyntaxKind::SqlcmdOperator)
                    .to_matchable()
            })
                .into()
        ),
        (
            "SqlcmdFilePathSegment".into(),
            TypedParser::new(SyntaxKind::UnquotedRelativeSqlFilePath, SyntaxKind::UnquotedRelativeSqlFilePath)
                .to_matchable()
                .into()
        ),
        (
            "FileCompressionSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(dialect.sets("file_compression").iter().map(|item| item.to_string()).collect_vec(), SyntaxKind::FileCompression)
                    .to_matchable()
            })
                .into()
        ),
        (
            "FileEncodingSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(dialect.sets("file_encoding").iter().map(|item| item.to_string()).collect_vec(), SyntaxKind::FileEncoding)
                    .to_matchable()
            })
                .into()
        ),
        (
            "SerdeMethodSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(dialect.sets("serde_method").iter().map(|item| item.to_string()).collect_vec(), SyntaxKind::SerdeMethod)
                    .to_matchable()
            })
                .into()
        ),
        (
            "ProcedureParameterGrammar".into(),
            Sequence::new(vec![Ref::new("ParameterNameSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::keyword("VARYING") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "DateFormatSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(dialect.sets("date_format").iter().map(|item| item.to_string()).collect_vec(), SyntaxKind::DateFormat)
                    .to_matchable()
            })
                .into()
        ),
        (
            "LeadingDotSegment".into(),
            StringParser::new(".", SyntaxKind::LeadingDot)
                .to_matchable()
                .into()
        ),
        (
            "HexadecimalLiteralSegment".into(),
            RegexParser::new(r#"([xX]'([\da-fA-F][\da-fA-F])+'|0[xX][\da-fA-F]*)"#, SyntaxKind::NumericLiteral)
                .to_matchable()
                .into()
        ),
        (
            "PlusComparisonSegment".into(),
            StringParser::new("+", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into()
        ),
        (
            "MinusComparisonSegment".into(),
            StringParser::new("-", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into()
        ),
        (
            "MultiplyComparisonSegment".into(),
            StringParser::new("*", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into()
        ),
        (
            "DivideComparisonSegment".into(),
            StringParser::new("/", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into()
        ),
        (
            "ModuloComparisonSegment".into(),
            StringParser::new("%", SyntaxKind::RawComparisonOperator)
                .to_matchable()
                .into()
        ),
        (
            "SizeLiteralSegment".into(),
            RegexParser::new(r#"\b\d+\s*?(KB|MB|GB|TB)\b"#, SyntaxKind::SizeLiteral)
                .to_matchable()
                .into()
        ),
        (
            "NakedOrQuotedIdentifierGrammar".into(),
            one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable(), Ref::new("BracketedIdentifierSegment") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "ActionParameterSegment".into(),
            RegexParser::new(r#"\$ACTION"#, SyntaxKind::ActionParameter)
                .to_matchable()
                .into()
        ),
    ]);
    
    tsql_dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");
                RegexParser::new(r#"[A-Z_\p{L}][A-Z0-9_@$#\p{L}]*"#, SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
                .into()
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                RegexParser::new(r#"[A-Z][A-Z0-9_]*|\[[A-Z][A-Z0-9_]*\]"#, SyntaxKind::FunctionNameIdentifier)
                    .anti_template(&format!("^({})$", dialect.sets("reserved_keywords").iter().filter(|item| !["UPDATE"].contains(item)).join("|")))
                    .to_matchable()
            })
                .into()
        ),
        (
            "DatatypeIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                one_of(vec![RegexParser::new(r#"[A-Z][A-Z0-9_]*|\[[A-Z][A-Z0-9_]*\]"#, SyntaxKind::DataTypeIdentifier) .anti_template(&format!("^({})$", dialect.sets("reserved_keywords").iter().join("|"))) .to_matchable(), Ref::new("SingleIdentifierGrammar") .exclude(Ref::new("NakedIdentifierSegment")) .to_matchable()])
                    .to_matchable()
            })
                .into()
        ),
    ]);
    
    tsql_dialect.replace_grammar(
        "QuotedIdentifierSegment",
        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "BaseExpressionElementGrammar",
        ansi_dialect.grammar("BaseExpressionElementGrammar").copy(None, None, None, Some(vec![Ref::new("IntervalExpressionSegment") .to_matchable()]), vec![], false)
    );
    
    tsql_dialect.replace_grammar(
        "SingleIdentifierGrammar",
        one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable(), Ref::new("BracketedIdentifierSegment") .to_matchable(), Ref::new("HashIdentifierSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "NumericLiteralSegment",
        one_of(vec![TypedParser::new(SyntaxKind::IntegerLiteral, SyntaxKind::NumericLiteral) .to_matchable(), TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral) .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "LiteralGrammar",
        ansi_dialect.grammar("LiteralGrammar").copy(Some(vec![Ref::new("QuotedLiteralSegmentWithN") .to_matchable(), Ref::new("IntegerLiteralSegment") .to_matchable(), Ref::new("BinaryLiteralSegment") .to_matchable()]), None, Some(Ref::new("NumericLiteralSegment") .to_matchable()), Some(vec![Ref::new("ArrayLiteralSegment") .to_matchable(), Ref::new("ObjectLiteralSegment") .to_matchable()]), vec![], false).copy(Some(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("SystemVariableSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    tsql_dialect.replace_grammar(
        "ParameterNameSegment",
        RegexParser::new(r#"@(?!@)[A-Za-z0-9_@$#]+"#, SyntaxKind::Parameter)
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "FunctionParameterGrammar",
        Sequence::new(vec![Ref::new("ParameterNameSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("NULL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "NanLiteralSegment",
        Nothing::new()
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "PrimaryKeyGrammar",
        Sequence::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("PRIMARY") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Ref::keyword("UNIQUE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "FromClauseTerminatorGrammar",
        one_of(vec![Ref::keyword("WHERE") .to_matchable(), Sequence::new(vec![Ref::keyword("GROUP") .to_matchable(), Ref::keyword("BY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Ref::keyword("BY") .to_matchable()]) .to_matchable(), Ref::keyword("HAVING") .to_matchable(), Ref::new("SetOperatorSegment") .to_matchable(), Ref::new("WithNoSchemaBindingClauseSegment") .to_matchable(), Ref::new("DelimiterGrammar") .to_matchable(), Ref::keyword("WINDOW") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "LikeGrammar",
        Sequence::new(vec![Ref::keyword("LIKE") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "FunctionContentsGrammar",
        AnyNumberOf::new(vec![Ref::new("ExpressionSegment") .to_matchable(), Sequence::new(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("DatetimeUnitSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Ref::keyword("FROM") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DISTINCT") .optional() .to_matchable(), one_of(vec![Ref::new("StarSegment") .to_matchable(), Delimited::new(vec![Ref::new("FunctionContentsExpressionGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("OrderByClauseSegment") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Ref::keyword("IN") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("IGNORE") .to_matchable(), Ref::keyword("RESPECT") .to_matchable()]) .to_matchable(), Ref::keyword("NULLS") .to_matchable()]) .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "JoinTypeKeywordsGrammar",
        Sequence::new(vec![one_of(vec![Ref::keyword("INNER") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FULL") .to_matchable(), Ref::keyword("LEFT") .to_matchable(), Ref::keyword("RIGHT") .to_matchable()]) .to_matchable(), Ref::keyword("OUTER") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("LOOP") .to_matchable(), Ref::keyword("HASH") .to_matchable(), Ref::keyword("MERGE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
            .config(|this| {
                this.optional();
            })
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "JoinKeywordsGrammar",
        one_of(vec![Ref::keyword("JOIN") .to_matchable(), Ref::keyword("APPLY") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "ConditionalCrossJoinKeywordsGrammar",
        Nothing::new()
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "NaturalJoinKeywordsGrammar",
        Ref::keyword("CROSS")
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "ExtendedNaturalJoinKeywordsGrammar",
        Sequence::new(vec![Ref::keyword("OUTER") .to_matchable(), Ref::keyword("APPLY") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "NestedJoinGrammar",
        Sequence::new(vec![MetaSegment::indent() .to_matchable(), Ref::new("JoinClauseSegment") .to_matchable(), MetaSegment::dedent() .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "Expression_D_Grammar",
        Sequence::new(vec![one_of(vec![Ref::new("BareFunctionSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Bracketed::new(vec![one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("SelectableGrammar") .to_matchable(), Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.parse_mode(ParseMode::Greedy); }) .to_matchable(), Ref::new("SelectStatementSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("TypedArrayLiteralSegment") .to_matchable(), Ref::new("ArrayLiteralSegment") .to_matchable(), Ref::keyword("DEFAULT") .to_matchable()]) .to_matchable(), Ref::new("AccessorGrammar") .optional() .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "MergeIntoLiteralGrammar",
        Sequence::new(vec![Ref::keyword("MERGE") .to_matchable(), Ref::new("TopPercentGrammar") .optional() .to_matchable(), Ref::keyword("INTO") .optional() .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "TrimParametersGrammar",
        Nothing::new()
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "TemporaryGrammar",
        Nothing::new()
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "JoinLikeClauseGrammar",
        AnyNumberOf::new(vec![Ref::new("PivotUnpivotStatementSegment") .to_matchable()])
            .config(|this| {
                this.max_times_per_element = Some(1);
                this.min_times(1);
            })
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "CollateGrammar",
        Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()])
            .to_matchable()
    );
    
    tsql_dialect.replace_grammar(
        "ArithmeticBinaryOperatorGrammar",
        ansi_dialect.grammar("ArithmeticBinaryOperatorGrammar").copy(Some(vec![Ref::new("AdditionAssignmentSegment") .to_matchable(), Ref::new("SubtractionAssignmentSegment") .to_matchable(), Ref::new("MultiplicationAssignmentSegment") .to_matchable(), Ref::new("DivisionAssignmentSegment") .to_matchable(), Ref::new("ModulusAssignmentSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    tsql_dialect.add([
        (
            "FileSegment".into(),
            NodeMatcher::new(SyntaxKind::File, |_dialect| {
                Sequence::new(vec![AnyNumberOf::new(vec![Ref::new("BatchSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BatchSegment".into(),
            NodeMatcher::new(SyntaxKind::Batch, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::new("OneOrMoreStatementsGrammar") .to_matchable(), Ref::new("BatchDelimiterGrammar") .optional() .to_matchable()]) .to_matchable(), Ref::new("BatchDelimiterGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GoStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::GoStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("GO") .to_matchable(), Ref::new("IntegerLiteralSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "StatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, |_dialect| {
                { let dialect = super::ansi::raw_dialect(); dialect.grammar("StatementSegment").match_grammar(&dialect).unwrap() }.copy(Some(vec![Ref::new("AlterDatabaseStatementSegment") .to_matchable(), Ref::new("CreateTableGraphStatementSegment") .to_matchable(), Ref::new("AlterTableSwitchStatementSegment") .to_matchable(), Ref::new("AlterIndexStatementSegment") .to_matchable(), Ref::new("CreateProcedureStatementSegment") .to_matchable(), Ref::new("DropProcedureStatementSegment") .to_matchable(), Ref::new("DropStatisticsStatementSegment") .to_matchable(), Ref::new("DisableTriggerStatementSegment") .to_matchable(), Ref::new("CreatePartitionFunctionSegment") .to_matchable(), Ref::new("AlterPartitionSchemeSegment") .to_matchable(), Ref::new("CreateMasterKeySegment") .to_matchable(), Ref::new("AlterMasterKeySegment") .to_matchable(), Ref::new("DropMasterKeySegment") .to_matchable(), Ref::new("CreateSecurityPolicySegment") .to_matchable(), Ref::new("AlterSecurityPolicySegment") .to_matchable(), Ref::new("DropSecurityPolicySegment") .to_matchable(), Ref::new("CreateSynonymStatementSegment") .to_matchable(), Ref::new("DropSynonymStatementSegment") .to_matchable(), Ref::new("CreateServerRoleStatementSegment") .to_matchable(), Ref::new("BulkInsertStatementSegment") .to_matchable(), Ref::new("MergeStatementSegment") .to_matchable(), Ref::new("BeginEndSegment") .to_matchable(), Ref::new("BreakStatement") .to_matchable(), Ref::new("ContinueStatement") .to_matchable(), Ref::new("GotoStatement") .to_matchable(), Ref::new("IfExpressionStatement") .to_matchable(), Ref::new("ReturnStatementSegment") .to_matchable(), Ref::new("ThrowStatementSegment") .to_matchable(), Ref::new("TryCatchSegment") .to_matchable(), Ref::new("WaitForStatementSegment") .to_matchable(), Ref::new("WhileExpressionStatement") .to_matchable(), Ref::new("DeclareCursorStatementSegment") .to_matchable(), Ref::new("OpenCursorStatementSegment") .to_matchable(), Ref::new("FetchCursorStatementSegment") .to_matchable(), Ref::new("CloseCursorStatementSegment") .to_matchable(), Ref::new("DeallocateCursorStatementSegment") .to_matchable(), Ref::new("PrintStatementSegment") .to_matchable(), Ref::new("RaiserrorStatementSegment") .to_matchable(), Ref::new("DeclareStatementSegment") .to_matchable(), Ref::new("ExecuteScriptSegment") .to_matchable(), Ref::new("SetStatementSegment") .to_matchable(), Ref::new("SetLanguageStatementSegment") .to_matchable(), Ref::new("SetLocalVariableStatementSegment") .to_matchable(), Ref::new("CreateTableAsSelectStatementSegment") .to_matchable(), Ref::new("RenameStatementSegment") .to_matchable(), Ref::new("UpdateStatisticsStatementSegment") .to_matchable(), Ref::new("LabelStatementSegment") .to_matchable(), Ref::new("CreateTypeStatementSegment") .to_matchable(), Ref::new("CreateDatabaseScopedCredentialStatementSegment") .to_matchable(), Ref::new("CreateExternalDataSourceStatementSegment") .to_matchable(), Ref::new("SqlcmdCommandSegment") .to_matchable(), Ref::new("CreateExternalFileFormat") .to_matchable(), Ref::new("CreateExternalTableStatementSegment") .to_matchable(), Ref::new("DropExternalTableStatementSegment") .to_matchable(), Ref::new("CopyIntoTableStatementSegment") .to_matchable(), Ref::new("CreateFullTextIndexStatementSegment") .to_matchable(), Ref::new("AtomicBeginEndSegment") .to_matchable(), Ref::new("ReconfigureStatementSegment") .to_matchable(), Ref::new("CreateColumnstoreIndexStatementSegment") .to_matchable(), Ref::new("CreatePartitionSchemeSegment") .to_matchable(), Ref::new("AlterPartitionFunctionSegment") .to_matchable(), Ref::new("OpenSymmetricKeySegment") .to_matchable(), Ref::new("CreateLoginStatementSegment") .to_matchable(), Ref::new("SetContextInfoSegment") .to_matchable(), Ref::new("CreateFullTextCatalogStatementSegment") .to_matchable()]), None, None, Some(vec![Ref::new("CreateCastStatementSegment") .to_matchable(), Ref::new("DropCastStatementSegment") .to_matchable(), Ref::new("CreateModelStatementSegment") .to_matchable(), Ref::new("DropModelStatementSegment") .to_matchable(), Ref::new("DescribeStatementSegment") .to_matchable(), Ref::new("ExplainStatementSegment") .to_matchable()]), vec![], false)
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::new("DatabaseReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONTAINMENT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("PARTIAL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Sequence::new(vec![Ref::keyword("PRIMARY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("FileSpecSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("FILEGROUP") .to_matchable(), Ref::new("NakedOrQuotedIdentifierGrammar") .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONTAINS") .to_matchable(), Ref::keyword("FILESTREAM") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CONTAINS") .to_matchable(), Ref::keyword("MEMORY_OPTIMIZED_DATA") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("FileSpecSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("LOG") .to_matchable(), Ref::keyword("ON") .to_matchable(), Delimited::new(vec![Ref::new("FileSpecSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("FILESTREAM") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("NON_TRANSACTED_ACCESS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("READ_ONLY") .to_matchable(), Ref::keyword("FULL") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DIRECTORY_NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_FULLTEXT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NESTED_TRIGGERS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("ON") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TRANSFORM_NOISE_WORDS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("ON") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TWO_DIGIT_YEAR_CUTOFF") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DB_CHAINING") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("ON") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TRUSTWORTHY") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("ON") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PERSISTENT_LOG_BUFFER") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("DIRECTORY_NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LEDGER") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CATALOG_COLLATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Delimited::new(vec![Ref::new("FileSpecSegment") .to_matchable()]) .to_matchable(), Ref::keyword("FOR") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ATTACH") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Ref::keyword("ENABLE_BROKER") .to_matchable(), Ref::keyword("NEW_BROKER") .to_matchable(), Ref::keyword("ERROR_BROKER_CONVERSATIONS") .to_matchable(), Ref::keyword("RESTRICTED_USER") .to_matchable(), Sequence::new(vec![Ref::keyword("FILESTREAM") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("DIRECTORY_NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("ATTACH_REBUILD_LOG") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Delimited::new(vec![Bracketed::new(vec![Sequence::new(vec![Ref::new("LogicalFileNameSegment") .optional() .to_matchable(), Ref::new("FileSpecFileNameSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::keyword("SNAPSHOT") .to_matchable(), Ref::keyword("OF") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), one_of(vec![Ref::new("DatabaseReferenceSegment") .to_matchable(), Ref::keyword("CURRENT") .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("MODIFY") .to_matchable(), Ref::keyword("NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("DatabaseReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MODIFY") .to_matchable(), Ref::new("BackupStorageRedundancySegment") .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("FILE") .to_matchable(), Ref::new("FileSpecSegmentInAlterDatabase") .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::keyword("FILEGROUP") .to_matchable(), Ref::new("NakedOrQuotedIdentifierGrammar") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("LOG") .to_matchable(), Ref::keyword("FILE") .to_matchable(), Delimited::new(vec![Ref::new("FileSpecSegmentInAlterDatabase") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REMOVE") .to_matchable(), Ref::keyword("FILE") .to_matchable(), Ref::new("LiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MODIFY") .to_matchable(), Ref::keyword("FILE") .to_matchable(), Ref::new("FileSpecSegmentInAlterDatabase") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("REMOVE") .to_matchable()]) .to_matchable(), Ref::keyword("FILEGROUP") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MODIFY") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("FileSpecMaxSizeSegment") .to_matchable(), Ref::new("EditionSegment") .to_matchable(), Ref::new("ServiceObjectiveSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("MANUAL_CUTOVER") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("CollateGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), one_of(vec![optionally_bracketed(vec![Delimited::new(vec![one_of(vec![Ref::new("CompatibilityLevelSegment") .to_matchable(), Ref::new("AutoOptionSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ACCELERATED_DATABASE_RECOVERY") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::keyword("PERSISTENT_VERSION_STORE_FILEGROUP") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NakedOrQuotedIdentifierGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILESTREAM") .to_matchable(), optionally_bracketed(vec![one_of(vec![Sequence::new(vec![Ref::keyword("NON_TRANSACTED_ACCESS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("READ_ONLY") .to_matchable(), Ref::keyword("FULL") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DIRECTORY_NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable(), Ref::keyword("LOCAL") .to_matchable(), Ref::keyword("NONE") .to_matchable(), Ref::keyword("DISABLED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("KB") .to_matchable(), Ref::keyword("MB") .to_matchable(), Ref::keyword("GB") .to_matchable(), Ref::keyword("TB") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RECOVERY") .to_matchable(), one_of(vec![Ref::keyword("FULL") .to_matchable(), Ref::keyword("SIMPLE") .to_matchable(), Ref::keyword("BULK_LOGGED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("REMOVE") .to_matchable()]) .to_matchable(), Ref::keyword("SECONDARY") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("SERVER") .to_matchable(), Ref::new("NakedOrQuotedIdentifierGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("AllowConnectionsSegment") .to_matchable(), Ref::new("ServiceObjectiveSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("PERFORM_CUTOVER") .to_matchable(), Ref::keyword("FAILOVER") .to_matchable(), Ref::keyword("FORCE_FAILOVER_ALLOW_DATA_LOSS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GreaterThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::new("RawGreaterThanSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("RawNotSegment") .to_matchable(), Ref::new("RawLessThanSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LessThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::new("RawLessThanSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("RawNotSegment") .to_matchable(), Ref::new("RawGreaterThanSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "NotEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::new("RawNotSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("RawLessThanSegment") .to_matchable(), Ref::new("RawGreaterThanSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LogicalFileNameSegment".into(),
            NodeMatcher::new(SyntaxKind::LogicalFileName, |_dialect| {
                Sequence::new(vec![Ref::keyword("NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecFileNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecFileName, |_dialect| {
                Sequence::new(vec![Ref::new("CommaSegment") .optional() .to_matchable(), Ref::keyword("FILENAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecNewNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecNewName, |_dialect| {
                Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("NEWNAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecSizeSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecSize, |_dialect| {
                Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("SIZE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("SizeLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("KB") .to_matchable(), Ref::keyword("MB") .to_matchable(), Ref::keyword("GB") .to_matchable(), Ref::keyword("TB") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecMaxSizeSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecMaxSize, |_dialect| {
                Sequence::new(vec![Ref::new("CommaSegment") .optional() .to_matchable(), Ref::keyword("MAXSIZE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("SizeLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("KB") .to_matchable(), Ref::keyword("MB") .to_matchable(), Ref::keyword("GB") .to_matchable(), Ref::keyword("TB") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("UNLIMITED") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecFileGrowthSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecFileGrowth, |_dialect| {
                Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("FILEGROWTH") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("SizeLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("KB") .to_matchable(), Ref::keyword("MB") .to_matchable(), Ref::keyword("GB") .to_matchable(), Ref::keyword("TB") .to_matchable(), Ref::new("PercentSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UnbracketedFileSpecSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpecWithoutBracket, |_dialect| {
                Sequence::new(vec![Ref::new("LogicalFileNameSegment") .optional() .to_matchable(), Ref::new("FileSpecFileNameSegment") .to_matchable(), Ref::new("FileSpecSizeSegment") .optional() .to_matchable(), Ref::new("FileSpecMaxSizeSegment") .optional() .to_matchable(), Ref::new("FileSpecFileGrowthSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecSegment".into(),
            NodeMatcher::new(SyntaxKind::FileSpec, |_dialect| {
                Bracketed::new(vec![Ref::new("UnbracketedFileSpecSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FileSpecSegmentInAlterDatabase".into(),
            NodeMatcher::new(SyntaxKind::FileSpec, |_dialect| {
                Bracketed::new(vec![get_unbracketed_file_spec_segment_grammar().copy(Some(vec![Ref::new("FileSpecNewNameSegment") .optional() .to_matchable(), Ref::new("FileSpecFileNameSegment") .optional() .to_matchable()]), None, Some(Ref::new("FileSpecSizeSegment") .optional() .to_matchable()), Some(vec![Ref::new("FileSpecFileNameSegment") .to_matchable()]), vec![], false)])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CollationReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::CollationReference, |_dialect| {
                one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::keyword("DATABASE_DEFAULT") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CompatibilityLevelSegment".into(),
            NodeMatcher::new(SyntaxKind::CompatibilityLevel, |_dialect| {
                Sequence::new(vec![Ref::keyword("COMPATIBILITY_LEVEL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AutoOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::AutoOption, |_dialect| {
                one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("AUTO_CLOSE") .to_matchable(), Ref::keyword("AUTO_SHRINK") .to_matchable(), Ref::keyword("AUTO_UPDATE_STATISTICS") .to_matchable(), Ref::keyword("AUTO_UPDATE_STATISTICS_ASYNC") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AUTO_CREATE_STATISTICS") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable(), Bracketed::new(vec![Ref::keyword("INCREMENTAL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ServiceObjectiveSegment".into(),
            NodeMatcher::new(SyntaxKind::ServiceObjective, |_dialect| {
                Sequence::new(vec![Ref::keyword("SERVICE_OBJECTIVE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ELASTIC_POOL") .to_matchable(), Bracketed::new(vec![Ref::keyword("NAME") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NakedOrQuotedIdentifierGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "EditionSegment".into(),
            NodeMatcher::new(SyntaxKind::Edition, |_dialect| {
                Sequence::new(vec![Ref::keyword("EDITION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AllowConnectionsSegment".into(),
            NodeMatcher::new(SyntaxKind::AllowConnections, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALLOW_CONNECTIONS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("NO") .to_matchable(), Ref::keyword("READ_ONLY") .to_matchable(), Ref::keyword("READ_WRITE") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BackupStorageRedundancySegment".into(),
            NodeMatcher::new(SyntaxKind::BackupStorageRedundancy, |_dialect| {
                Sequence::new(vec![Ref::keyword("BACKUP_STORAGE_REDUNDANCY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CursorDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::CursorDefinition, |_dialect| {
                Sequence::new(vec![Ref::keyword("CURSOR") .to_matchable(), one_of(vec![Ref::keyword("LOCAL") .to_matchable(), Ref::keyword("GLOBAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("FORWARD_ONLY") .to_matchable(), Ref::keyword("SCROLL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("STATIC") .to_matchable(), Ref::keyword("KEYSET") .to_matchable(), Ref::keyword("DYNAMIC") .to_matchable(), Ref::keyword("FAST_FORWARD") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("READ_ONLY") .to_matchable(), Ref::keyword("SCROLL_LOCKS") .to_matchable(), Ref::keyword("OPTIMISTIC") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("TYPE_WARNING") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("SelectStatementSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectVariableAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectVariableAssignment, |_dialect| {
                Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("AssignmentOperatorSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectClauseElementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseElement, |_dialect| {
                one_of(vec![Ref::new("WildcardExpressionSegment") .to_matchable(), Ref::new("SelectVariableAssignmentSegment") .to_matchable(), Sequence::new(vec![Ref::new("AltAliasExpressionSegment") .to_matchable(), Ref::new("BaseExpressionElementGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("BaseExpressionElementGrammar") .to_matchable(), Ref::new("AliasExpressionSegment") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AltAliasExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasExpression, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable(), Ref::new("BracketedIdentifierSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable()]) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("EqualAliasOperatorSegment") .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "EqualAliasOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasOperator, |_dialect| {
                Sequence::new(vec![Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectClauseModifierSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseModifier, |_dialect| {
                AnyNumberOf::new(vec![Ref::keyword("DISTINCT") .to_matchable(), Ref::keyword("ALL") .to_matchable(), Sequence::new(vec![Ref::keyword("TOP") .to_matchable(), optionally_bracketed(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PERCENT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("TIES") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("SELECT") .to_matchable(), Ref::new("SelectClauseModifierSegment") .optional() .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![Ref::new("SelectClauseElementSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UnorderedSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                Sequence::new(vec![Ref::new("SelectClauseSegment") .to_matchable(), Ref::new("IntoTableSegment") .optional() .to_matchable(), Ref::new("FromClauseSegment") .optional() .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Ref::new("GroupByClauseSegment") .optional() .to_matchable(), Ref::new("HavingClauseSegment") .optional() .to_matchable(), Ref::new("NamedWindowSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("INSERT") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("INTO") .optional() .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("OpenQuerySegment") .to_matchable()]) .to_matchable(), Ref::new("PostTableExpressionGrammar") .optional() .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), Ref::new("OutputClauseSegment") .optional() .to_matchable(), one_of(vec![Ref::new("SelectableGrammar") .to_matchable(), Ref::new("ExecuteScriptSegment") .to_matchable(), Ref::new("DefaultValuesGrammar") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BulkInsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::BulkInsertStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("BULK") .to_matchable(), Ref::keyword("INSERT") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("FROM") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("BulkInsertStatementWithSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BulkInsertStatementWithSegment".into(),
            NodeMatcher::new(SyntaxKind::BulkInsertWithSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("BATCHSIZE") .to_matchable(), Ref::keyword("FIRSTROW") .to_matchable(), Ref::keyword("KILOBYTES_PER_BATCH") .to_matchable(), Ref::keyword("LASTROW") .to_matchable(), Ref::keyword("MAXERRORS") .to_matchable(), Ref::keyword("ROWS_PER_BATCH") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("CODEPAGE") .to_matchable(), Ref::keyword("DATAFILETYPE") .to_matchable(), Ref::keyword("DATA_SOURCE") .to_matchable(), Ref::keyword("ERRORFILE") .to_matchable(), Ref::keyword("ERRORFILE_DATA_SOURCE") .to_matchable(), Ref::keyword("FORMATFILE_DATA_SOURCE") .to_matchable(), Ref::keyword("ROWTERMINATOR") .to_matchable(), Ref::keyword("FORMAT") .to_matchable(), Ref::keyword("FIELDQUOTE") .to_matchable(), Ref::keyword("FORMATFILE") .to_matchable(), Ref::keyword("FIELDTERMINATOR") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("ASC") .to_matchable(), Ref::keyword("DESC") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("CHECK_CONSTRAINTS") .to_matchable(), Ref::keyword("FIRE_TRIGGERS") .to_matchable(), Ref::keyword("KEEPIDENTITY") .to_matchable(), Ref::keyword("KEEPNULLS") .to_matchable(), Ref::keyword("TABLOCK") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WithCompoundStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCompoundStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("RECURSIVE") .optional() .to_matchable(), Conditional::new(MetaSegment::indent()) .indented_ctes() .to_matchable(), Delimited::new(vec![Ref::new("CTEDefinitionSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("SELECT") .to_matchable()]; }) .to_matchable(), Conditional::new(MetaSegment::dedent()) .indented_ctes() .to_matchable(), one_of(vec![Ref::new("NonWithSelectableGrammar") .to_matchable(), Ref::new("NonWithNonSelectableGrammar") .to_matchable(), Ref::new("MergeStatementSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                get_unordered_select_statement_segment_grammar().copy(Some(vec![Ref::new("OrderByClauseSegment") .optional() .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable(), Ref::new("ForClauseSegment") .optional() .to_matchable()]), None, None, None, vec![], false)
            })
                .to_matchable()
                .into()
        ),
        (
            "IntoTableSegment".into(),
            NodeMatcher::new(SyntaxKind::IntoTableClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("INTO") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WhereClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WhereClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WHERE") .to_matchable(), MetaSegment::implicit_indent() .to_matchable(), optionally_bracketed(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("OrReplaceGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("UNIQUE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("STATISTICS") .to_matchable()]) .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedIndexColumnListGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("INCLUDE") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Ref::new("RelationalIndexOptionsSegment") .optional() .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable(), Ref::new("FilestreamOnOptionSegment") .optional() .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateColumnstoreIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateColumnstoreIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedIndexColumnListGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("DROP_EXISTING") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MAXDOP") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ONLINE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COMPRESSION_DELAY") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("COLUMNSTORE_ARCHIVE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateFullTextIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFulltextIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("FULLTEXT") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("TYPE") .to_matchable(), Ref::keyword("COLUMN") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LANGUAGE") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("STATISTICAL_SEMANTICS") .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("KEY") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Delimited::new(vec![AnyNumberOf::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("FILEGROUP") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("CHANGE_TRACKING") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("MANUAL") .to_matchable(), Ref::keyword("AUTO") .to_matchable(), Delimited::new(vec![Ref::keyword("OFF") .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("POPULATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("STOPLIST") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Ref::keyword("SYSTEM") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SEARCH") .to_matchable(), Ref::keyword("PROPERTY") .to_matchable(), Ref::keyword("LIST") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), one_of(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("ALL") .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("REBUILD") .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ALL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("PAD_INDEX") .to_matchable(), Ref::keyword("SORT_IN_TEMPDB") .to_matchable(), Ref::keyword("IGNORE_DUP_KEY") .to_matchable(), Ref::keyword("STATISTICS_NORECOMPUTE") .to_matchable(), Ref::keyword("STATISTICS_INCREMENTAL") .to_matchable(), Ref::keyword("RESUMABLE") .to_matchable(), Ref::keyword("ALLOW_ROW_LOCKS") .to_matchable(), Ref::keyword("ALLOW_PAGE_LOCKS") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("MAXDOP") .to_matchable(), Ref::keyword("FILLFACTOR") .to_matchable(), Ref::keyword("MAX_DURATION") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ONLINE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("WAIT_AT_LOW_PRIORITY") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ABORT_AFTER_WAIT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("BLOCKERS") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("ROW") .to_matchable(), Ref::keyword("PAGE") .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("COLUMNSTORE_ARCHIVE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("XML_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("XML_COMPRESSION") .to_matchable(), Ref::keyword("SORT_IN_TEMPDB") .to_matchable(), Ref::keyword("RESUMABLE") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("MAXDOP") .to_matchable(), Ref::keyword("MAX_DURATION") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("ROW") .to_matchable(), Ref::keyword("PAGE") .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("COLUMNSTORE_ARCHIVE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ONLINE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("WAIT_AT_LOW_PRIORITY") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ABORT_AFTER_WAIT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("BLOCKERS") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("DISABLE") .to_matchable(), Sequence::new(vec![Ref::keyword("REORGANIZE") .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("LOB_COMPACTION") .to_matchable(), Ref::keyword("COMPRESS_ALL_ROW_GROUPS") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("ALLOW_ROW_LOCKS") .to_matchable(), Ref::keyword("ALLOW_PAGE_LOCKS") .to_matchable(), Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY") .to_matchable(), Ref::keyword("IGNORE_DUP_KEY") .to_matchable(), Ref::keyword("STATISTICS_NORECOMPUTE") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COMPRESSION_DELAY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RESUME") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::keyword("MAXDOP") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WAIT_AT_LOW_PRIORITY") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ABORT_AFTER_WAIT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("BLOCKERS") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("PAUSE") .to_matchable(), Ref::keyword("ABORT") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OnPartitionOrFilegroupOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::OnPartitionOrFilegroupStatement, |_dialect| {
                one_of(vec![Ref::new("PartitionSchemeClause") .to_matchable(), Ref::new("FilegroupClause") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FilestreamOnOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::FilestreamOnOptionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("FILESTREAM_ON") .to_matchable(), one_of(vec![Ref::new("FilegroupNameSegment") .to_matchable(), Ref::new("PartitionSchemeNameSegment") .to_matchable(), one_of(vec![Ref::keyword("NULL") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TextimageOnOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::TextimageOnOptionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("TEXTIMAGE_ON") .to_matchable(), one_of(vec![Ref::new("FilegroupNameSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableOptionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("MEMORY_OPTIMIZED") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ON") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DURABILITY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("SCHEMA_ONLY") .to_matchable(), Ref::keyword("SCHEMA_AND_DATA") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SYSTEM_VERSIONING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("HISTORY_TABLE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("HISTORY_RETENTION_PERIOD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("INFINITE") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DAYS") .to_matchable(), Ref::keyword("WEEKS") .to_matchable(), Ref::keyword("MONTHS") .to_matchable(), Ref::keyword("YEARS") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("DATA_CONSISTENCY_CHECK") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("ROW") .to_matchable(), Ref::keyword("PAGE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("XML_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILETABLE_DIRECTORY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FILETABLE_COLLATE_FILENAME") .to_matchable(), Ref::keyword("FILETABLE_PRIMARY_KEY_CONSTRAINT_NAME") .to_matchable(), Ref::keyword("FILETABLE_STREAMID_UNIQUE_CONSTRAINT_NAME") .to_matchable(), Ref::keyword("FILETABLE_FULLPATH_UNIQUE_CONSTRAINT_NAME") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REMOTE_DATA_ARCHIVE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("FILTER_PREDICATE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NULL") .to_matchable(), Ref::new("FunctionNameSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("MIGRATION_STATE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OUTBOUND") .to_matchable(), Ref::keyword("INBOUND") .to_matchable(), Ref::keyword("PAUSED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OFF") .to_matchable(), Bracketed::new(vec![Ref::keyword("MIGRATION_STATE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("PAUSED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_DELETION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Ref::keyword("FILTER_COLUMN") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::keyword("RETENTION_PERIOD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .optional() .to_matchable(), Ref::new("DatetimeUnitSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LEDGER") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("LEDGER_VIEW") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("TRANSACTION_ID_COLUMN_NAME") .to_matchable(), Ref::keyword("SEQUENCE_NUMBER_COLUMN_NAME") .to_matchable(), Ref::keyword("OPERATION_TYPE_COLUMN_NAME") .to_matchable(), Ref::keyword("OPERATION_TYPE_DESC_COLUMN_NAME") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("APPEND_ONLY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReferencesConstraintGrammar".into(),
            NodeMatcher::new(SyntaxKind::ReferencesConstraintGrammar, |_dialect| {
                Sequence::new(vec![Ref::keyword("REFERENCES") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("DELETE") .to_matchable(), Ref::new("ReferentialActionGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Ref::new("ReferentialActionGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CheckConstraintGrammar".into(),
            NodeMatcher::new(SyntaxKind::CheckConstraintGrammar, |_dialect| {
                Sequence::new(vec![Ref::keyword("CHECK") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ConnectionConstraintGrammar".into(),
            NodeMatcher::new(SyntaxKind::ConnectionConstraintGrammar, |_dialect| {
                Sequence::new(vec![Ref::keyword("CONNECTION") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("DELETE") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("ACTION") .to_matchable()]) .to_matchable(), Ref::keyword("CASCADE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("ACTION") .to_matchable()]) .to_matchable(), Ref::keyword("CASCADE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RelationalIndexOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::RelationalIndexOptions, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), optionally_bracketed(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("PAD_INDEX") .to_matchable(), Ref::keyword("FILLFACTOR") .to_matchable(), Ref::keyword("SORT_IN_TEMPDB") .to_matchable(), Ref::keyword("IGNORE_DUP_KEY") .to_matchable(), Ref::keyword("STATISTICS_NORECOMPUTE") .to_matchable(), Ref::keyword("STATISTICS_INCREMENTAL") .to_matchable(), Ref::keyword("DROP_EXISTING") .to_matchable(), Ref::keyword("RESUMABLE") .to_matchable(), Ref::keyword("ALLOW_ROW_LOCKS") .to_matchable(), Ref::keyword("ALLOW_PAGE_LOCKS") .to_matchable(), Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY") .to_matchable(), Ref::keyword("MAXDOP") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("MaxDurationSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ONLINE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("OFF") .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("WAIT_AT_LOW_PRIORITY") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("MaxDurationSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ABORT_AFTER_WAIT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("BLOCKERS") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COMPRESSION_DELAY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("MINUTES") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("ROW") .to_matchable(), Ref::keyword("PAGE") .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("COLUMNSTORE_ARCHIVE") .to_matchable()]) .to_matchable(), Ref::new("OnPartitionsSegment") .optional() .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(1); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MaxDurationSegment".into(),
            NodeMatcher::new(SyntaxKind::MaxDuration, |_dialect| {
                Sequence::new(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("MINUTES") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropStatisticsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), one_of(vec![Ref::keyword("STATISTICS") .to_matchable()]) .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UpdateStatisticsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatisticsStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("STATISTICS") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Ref::keyword("FULLSCAN") .to_matchable(), Ref::keyword("RESAMPLE") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReconfigureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReconfigureStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RECONFIGURE") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("OVERRIDE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ObjectReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ObjectReference, |_dialect| {
                Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::new("DotSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(0); this.max_times(3); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::new("DotSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(0); this.max_times(3); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("LeadingDotSegment") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .optional() .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(0); this.max_times(2); }) .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SchemaReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::SchemaReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "DatabaseReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "IndexReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExtensionReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ExtensionReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "ColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "SequenceReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::SequenceReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "PivotColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotColumnReference, |_dialect| {
                _dialect.grammar("ObjectReferenceSegment").match_grammar(&_dialect).unwrap()
            })
                .to_matchable()
                .into()
        ),
        (
            "PivotUnpivotStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FromPivotExpression, |_dialect| {
                Sequence::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("PIVOT") .to_matchable(), optionally_bracketed(vec![Sequence::new(vec![optionally_bracketed(vec![Ref::new("FunctionSegment") .to_matchable()]) .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::keyword("IN") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("PivotColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("UNPIVOT") .to_matchable(), optionally_bracketed(vec![Sequence::new(vec![optionally_bracketed(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::keyword("IN") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("PivotColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("DatatypeSegment") .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("CURSOR") .to_matchable(), Sequence::new(vec![Ref::keyword("TABLE") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableConstraintSegment") .to_matchable(), Ref::new("ComputedColumnDefinitionSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeclareCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable(), one_of(vec![Ref::new("CursorDefinitionSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("INSENSITIVE") .optional() .to_matchable(), Ref::keyword("SCROLL") .optional() .to_matchable(), Ref::keyword("CURSOR") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("SelectStatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BracketedArguments".into(),
            NodeMatcher::new(SyntaxKind::BracketedArguments, |_dialect| {
                Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::keyword("MAX") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .config(|this| { this.optional(); this.disallow_gaps(); }) .to_matchable(), one_of(vec![Bracketed::new(vec![Ref::new("DatatypeIdentifierSegment") .to_matchable()]) .config(|this| { this.bracket_type("square"); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("TINYINT") .to_matchable(), Ref::keyword("SMALLINT") .to_matchable(), Ref::keyword("INT") .to_matchable(), Ref::keyword("BIGINT") .to_matchable(), Ref::keyword("BIT") .to_matchable(), Ref::keyword("MONEY") .to_matchable(), Ref::keyword("SMALLMONEY") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DECIMAL") .to_matchable(), Ref::keyword("NUMERIC") .to_matchable(), Ref::keyword("DEC") .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FLOAT") .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Ref::keyword("REAL") .to_matchable(), Ref::keyword("DATE") .to_matchable(), Ref::keyword("SMALLDATETIME") .to_matchable(), Ref::keyword("DATETIME") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("TIME") .to_matchable(), Ref::keyword("DATETIME2") .to_matchable(), Ref::keyword("DATETIMEOFFSET") .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("CHAR") .to_matchable(), Ref::keyword("CHARACTER") .to_matchable()]) .to_matchable(), Ref::keyword("VARYING") .optional() .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("VARCHAR") .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Ref::keyword("TEXT") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("NCHAR") .to_matchable(), Sequence::new(vec![Ref::keyword("NATIONAL") .to_matchable(), one_of(vec![Ref::keyword("CHAR") .to_matchable(), Ref::keyword("CHARACTER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("VARYING") .optional() .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("NVARCHAR") .to_matchable(), Sequence::new(vec![Ref::keyword("NATIONAL") .to_matchable(), Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("VARYING") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Ref::keyword("NTEXT") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("VARBINARY") .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable(), Ref::keyword("IMAGE") .to_matchable(), Ref::keyword("CURSOR") .to_matchable(), Ref::keyword("SQL_VARIANT") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::keyword("TIMESTAMP") .to_matchable(), Ref::keyword("ROWVERSION") .to_matchable(), Ref::keyword("UNIQUEIDENTIFIER") .to_matchable(), Ref::keyword("XML") .to_matchable(), Ref::keyword("JSON") .to_matchable(), Ref::keyword("GEOGRAPHY") .to_matchable(), Ref::keyword("GEOMETRY") .to_matchable(), Ref::keyword("HIERARCHYID") .to_matchable(), Sequence::new(vec![Ref::keyword("VECTOR") .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("DatatypeIdentifierSegment") .to_matchable()]) .to_matchable(), Ref::new("CharCharacterSetGrammar") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateSequenceOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSequenceOptionsSegment, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("START") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("INCREMENT") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MINVALUE") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("MINVALUE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MAXVALUE") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("MAXVALUE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("NO") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("CYCLE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CACHE") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("CACHE") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "NextValueSequenceSegment".into(),
            NodeMatcher::new(SyntaxKind::SequenceNextValue, |_dialect| {
                Sequence::new(vec![Ref::keyword("NEXT") .to_matchable(), Ref::keyword("VALUE") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IfExpressionStatement".into(),
            NodeMatcher::new(SyntaxKind::IfThenStatement, |_dialect| {
                Sequence::new(vec![Ref::new("IfClauseSegment") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("StatementAndDelimiterGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("ELSE") .to_matchable(), Ref::new("IfClauseSegment") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("StatementAndDelimiterGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ELSE") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("StatementAndDelimiterGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IfClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::IfClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("IF") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WhileExpressionStatement".into(),
            NodeMatcher::new(SyntaxKind::WhileStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("WHILE") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("StatementAndDelimiterGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BreakStatement".into(),
            NodeMatcher::new(SyntaxKind::BreakStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("BREAK") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ContinueStatement".into(),
            NodeMatcher::new(SyntaxKind::ContinueStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CONTINUE") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WaitForStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WaitforStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("WAITFOR") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("DELAY") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TIME") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TIMEOUT") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("FILESTREAM") .to_matchable(), Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .to_matchable(), Ref::keyword("SPARSE") .to_matchable(), Sequence::new(vec![Ref::keyword("MASKED") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Ref::keyword("FUNCTION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("DEFAULT") .to_matchable(), optionally_bracketed(vec![one_of(vec![optionally_bracketed(vec![Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable(), Ref::new("BareFunctionSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Ref::new("NextValueSequenceSegment") .to_matchable(), Ref::new("HexadecimalLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("IdentityGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("GENERATED") .to_matchable(), Ref::keyword("ALWAYS") .to_matchable(), Ref::keyword("AS") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ROW") .to_matchable(), Ref::keyword("TRANSACTION_ID") .to_matchable(), Ref::keyword("SEQUENCE_NUMBER") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("START") .to_matchable(), Ref::keyword("END") .to_matchable()]) .to_matchable(), Ref::keyword("HIDDEN") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .optional() .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Ref::keyword("ROWGUIDCOL") .to_matchable(), Ref::new("EncryptedWithGrammar") .to_matchable(), Ref::new("PrimaryKeyGrammar") .to_matchable(), Ref::new("RelationalIndexOptionsSegment") .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .to_matchable(), Ref::new("ForeignKeyGrammar") .to_matchable(), Ref::new("ReferencesConstraintGrammar") .to_matchable(), Ref::new("CheckConstraintGrammar") .to_matchable(), Ref::new("FilestreamOnOptionSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("INDEX") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("TableConstraintSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FunctionParameterListGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionParameterList, |_dialect| {
                Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("FunctionParameterGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("READONLY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("ALTER") .to_matchable(), Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("OR") .to_matchable(), Ref::keyword("ALTER") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::new("FunctionParameterListGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("RETURNS") .to_matchable(), one_of(vec![Ref::new("DatatypeSegment") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("TableConstraintSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("FunctionOptionSegment") .optional() .to_matchable(), Ref::keyword("AS") .optional() .to_matchable(), Ref::new("ProcedureDefinitionGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FunctionOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionOptionSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![AnyNumberOf::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("SCHEMABINDING") .to_matchable(), Sequence::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("RETURNS") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Ref::keyword("CALLED") .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable(), Ref::keyword("INPUT") .to_matchable()]) .to_matchable(), Ref::new("ExecuteAsClauseSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("INLINE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(1); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Delimited::new(vec![Ref::new("FunctionNameSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReturnStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReturnSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("RETURN") .to_matchable(), Ref::new("ExpressionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExecuteAsClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteAsClause, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("EXEC") .to_matchable(), Ref::keyword("EXECUTE") .to_matchable()]) .to_matchable(), Ref::keyword("AS") .to_matchable(), one_of(vec![Ref::keyword("CALLER") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("OWNER") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetLocalVariableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetLocalVariableSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("AssignmentOperatorSegment") .to_matchable(), one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("CursorDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("TRANSACTION") .to_matchable(), Ref::keyword("ISOLATION") .to_matchable(), Ref::keyword("LEVEL") .to_matchable(), one_of(vec![Ref::keyword("SNAPSHOT") .to_matchable(), Ref::keyword("SERIALIZABLE") .to_matchable(), Sequence::new(vec![Ref::keyword("REPEATABLE") .to_matchable(), Ref::keyword("READ") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("READ") .to_matchable(), one_of(vec![Ref::keyword("COMMITTED") .to_matchable(), Ref::keyword("UNCOMMITTED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Delimited::new(vec![Ref::keyword("DATEFIRST") .to_matchable(), Ref::keyword("DATEFORMAT") .to_matchable(), Ref::keyword("DEADLOCK_PRIORITY") .to_matchable(), Ref::keyword("LOCK_TIMEOUT") .to_matchable(), Ref::keyword("CONCAT_NULL_YIELDS_NULL") .to_matchable(), Ref::keyword("CURSOR_CLOSE_ON_COMMIT") .to_matchable(), Ref::keyword("FIPS_FLAGGER") .to_matchable(), Sequence::new(vec![Ref::keyword("IDENTITY_INSERT") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Ref::keyword("LANGUAGE") .to_matchable(), Ref::keyword("OFFSETS") .to_matchable(), Ref::keyword("QUOTED_IDENTIFIER") .to_matchable(), Ref::keyword("ARITHABORT") .to_matchable(), Ref::keyword("ARITHIGNORE") .to_matchable(), Ref::keyword("FMTONLY") .to_matchable(), Ref::keyword("NOCOUNT") .to_matchable(), Ref::keyword("NOEXEC") .to_matchable(), Ref::keyword("NUMERIC_ROUNDABORT") .to_matchable(), Ref::keyword("PARSEONLY") .to_matchable(), Ref::keyword("QUERY_GOVERNOR_COST_LIMIT") .to_matchable(), Ref::keyword("RESULT_SET_CACHING") .to_matchable(), Ref::keyword("ROWCOUNT") .to_matchable(), Ref::keyword("TEXTSIZE") .to_matchable(), Ref::keyword("ANSI_DEFAULTS") .to_matchable(), Ref::keyword("ANSI_NULL_DFLT_OFF") .to_matchable(), Ref::keyword("ANSI_NULL_DFLT_ON") .to_matchable(), Ref::keyword("ANSI_NULLS") .to_matchable(), Ref::keyword("ANSI_PADDING") .to_matchable(), Ref::keyword("ANSI_WARNINGS") .to_matchable(), Ref::keyword("FORCEPLAN") .to_matchable(), Ref::keyword("SHOWPLAN_ALL") .to_matchable(), Ref::keyword("SHOWPLAN_TEXT") .to_matchable(), Ref::keyword("SHOWPLAN_XML") .to_matchable(), Sequence::new(vec![Ref::keyword("STATISTICS") .to_matchable(), one_of(vec![Ref::keyword("IO") .to_matchable(), Ref::keyword("PROFILE") .to_matchable(), Ref::keyword("TIME") .to_matchable(), Ref::keyword("XML") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("IMPLICIT_TRANSACTIONS") .to_matchable(), Ref::keyword("REMOTE_PROC_TRANSACTIONS") .to_matchable(), Ref::keyword("XACT_ABORT") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Ref::keyword("LOW") .to_matchable(), Ref::keyword("NORMAL") .to_matchable(), Ref::keyword("HIGH") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QualifiedNumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AssignmentOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_dialect| {
                one_of(vec![Ref::new("RawEqualsSegment") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("PlusSegment") .to_matchable(), Ref::new("MinusSegment") .to_matchable(), Ref::new("DivideSegment") .to_matchable(), Ref::new("MultiplySegment") .to_matchable(), Ref::new("ModuloSegment") .to_matchable(), Ref::new("BitwiseAndSegment") .to_matchable(), Ref::new("BitwiseOrSegment") .to_matchable(), Ref::new("BitwiseXorSegment") .to_matchable()]) .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()]) .config(|this| { this.disallow_gaps(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ProcedureParameterListGrammar".into(),
            NodeMatcher::new(SyntaxKind::ProcedureParameterList, |_dialect| {
                optionally_bracketed(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ProcedureParameterGrammar") .to_matchable(), one_of(vec![Ref::keyword("OUT") .to_matchable(), Ref::keyword("OUTPUT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("READONLY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("ALTER") .to_matchable(), Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("OR") .to_matchable(), Ref::keyword("ALTER") .to_matchable()]) .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("PROC") .to_matchable(), Ref::keyword("PROCEDURE") .to_matchable()]) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("SemicolonSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("ProcedureParameterListGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![AnyNumberOf::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("RECOMPILE") .to_matchable(), Ref::keyword("NATIVE_COMPILATION") .to_matchable(), Ref::keyword("SCHEMABINDING") .to_matchable(), Ref::new("ExecuteAsClauseSegment") .optional() .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("ProcedureDefinitionGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), one_of(vec![Ref::keyword("PROCEDURE") .to_matchable(), Ref::keyword("PROC") .to_matchable()]) .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ProcedureDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::ProcedureStatement, |_dialect| {
                one_of(vec![Ref::new("OneOrMoreStatementsGrammar") .to_matchable(), Ref::new("AtomicBeginEndSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("NAME") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateViewStatement, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("ALTER") .to_matchable(), Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("OR") .to_matchable(), Ref::keyword("ALTER") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("IndexColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("SCHEMABINDING") .to_matchable(), Ref::keyword("VIEW_METADATA") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("CHECK") .to_matchable(), Ref::keyword("OPTION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MLTableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::MlTableExpression, |_dialect| {
                Nothing::new()
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ConvertFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("CONVERT") .to_matchable(), Ref::keyword("TRY_CONVERT") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CastFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                Sequence::new(vec![Ref::keyword("CAST") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReplicateFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                Sequence::new(vec![Ref::keyword("REPLICATE") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "JsonFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("JSON_ARRAY") .to_matchable(), Ref::keyword("JSON_OBJECT") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RankFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("DENSE_RANK") .to_matchable(), Ref::keyword("NTILE") .to_matchable(), Ref::keyword("RANK") .to_matchable(), Ref::keyword("ROW_NUMBER") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReservedKeywordFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("COALESCE") .to_matchable(), Ref::keyword("LEFT") .to_matchable(), Ref::keyword("NULLIF") .to_matchable(), Ref::keyword("RIGHT") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReservedKeywordBareFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("CURRENT_TIMESTAMP") .to_matchable(), Ref::keyword("CURRENT_USER") .to_matchable(), Ref::keyword("SESSION_USER") .to_matchable(), Ref::keyword("SYSTEM_USER") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WithinGroupFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_dialect| {
                one_of(vec![Ref::keyword("STRING_AGG") .to_matchable(), Ref::keyword("PERCENTILE_CONT") .to_matchable(), Ref::keyword("PERCENTILE_DISC") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WithinGroupClause".into(),
            NodeMatcher::new(SyntaxKind::WithinGroupClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITHIN") .to_matchable(), Ref::keyword("GROUP") .to_matchable(), Bracketed::new(vec![Ref::new("OrderByClauseSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OVER") .to_matchable(), Bracketed::new(vec![Ref::new("PartitionClauseSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PartitionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionbyClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("BY") .to_matchable(), Delimited::new(vec![optionally_bracketed(vec![one_of(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OnPartitionsSegment".into(),
            NodeMatcher::new(SyntaxKind::OnPartitionsClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("PARTITIONS") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PartitionSchemeNameSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionSchemeName, |_dialect| {
                Ref::new("SingleIdentifierGrammar")
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PartitionSchemeClause".into(),
            NodeMatcher::new(SyntaxKind::PartitionSchemeClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::new("PartitionSchemeNameSegment") .to_matchable(), Bracketed::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CastFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_dialect| {
                Sequence::new(vec![Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ConvertFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_dialect| {
                Sequence::new(vec![Bracketed::new(vec![Ref::new("DatatypeSegment") .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReplicateFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_dialect| {
                Sequence::new(vec![Bracketed::new(vec![one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("HexadecimalLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "JsonFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_dialect| {
                one_of(vec![Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::keyword("NULL") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("NULL") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ABSENT") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Ref::new("ColonSegment") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Bracketed::new(vec![Ref::new("SelectStatementSegment") .to_matchable()]) .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("NULL") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ABSENT") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("NULL") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ABSENT") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RankFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_dialect| {
                Sequence::new(vec![Bracketed::new(vec![Ref::new("NumericLiteralSegment") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::Function, |_dialect| {
                one_of(vec![Ref::new("ReservedKeywordBareFunctionNameSegment") .to_matchable(), Sequence::new(vec![Ref::new("DatePartFunctionNameSegment") .to_matchable(), Ref::new("DateTimeFunctionContentsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("RankFunctionNameSegment") .to_matchable(), Ref::new("RankFunctionContentsSegment") .to_matchable(), Ref::new("OverClauseSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ConvertFunctionNameSegment") .to_matchable(), Ref::new("ConvertFunctionContentsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("CastFunctionNameSegment") .to_matchable(), Ref::new("CastFunctionContentsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ReplicateFunctionNameSegment") .to_matchable(), Ref::new("ReplicateFunctionContentsSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("WithinGroupFunctionNameSegment") .to_matchable(), Ref::new("FunctionContentsSegment") .to_matchable(), Ref::new("WithinGroupClause") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("FunctionNameSegment") .exclude(one_of(vec![Ref::new("ValuesClauseSegment") .to_matchable(), Ref::new("CastFunctionNameSegment") .to_matchable(), Ref::new("ConvertFunctionNameSegment") .to_matchable(), Ref::new("DatePartFunctionNameSegment") .to_matchable(), Ref::new("WithinGroupFunctionNameSegment") .to_matchable(), Ref::new("RankFunctionNameSegment") .to_matchable()])) .to_matchable(), Ref::new("ReservedKeywordFunctionNameSegment") .to_matchable()]) .to_matchable(), Ref::new("FunctionContentsSegment") .to_matchable(), Ref::new("PostFunctionGrammar") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("JsonFunctionNameSegment") .to_matchable(), Ref::new("JsonFunctionContentsSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), one_of(vec![Bracketed::new(vec![Delimited::new(vec![Ref::new("TableConstraintSegment") .to_matchable(), Ref::new("ComputedColumnDefinitionSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), Ref::new("TableIndexSegment") .to_matchable(), Ref::new("PeriodSegment") .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LIKE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("TableDistributionIndexClause") .optional() .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable(), Ref::new("FilestreamOnOptionSegment") .optional() .to_matchable(), Ref::new("TextimageOnOptionSegment") .optional() .to_matchable(), Ref::new("TableOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTableGraphStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableGraphStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("GraphTableConstraintSegment") .to_matchable(), Ref::new("ComputedColumnDefinitionSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), Ref::new("TableIndexSegment") .to_matchable(), Ref::new("PeriodSegment") .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), one_of(vec![Ref::keyword("NODE") .to_matchable(), Ref::keyword("EDGE") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("COLUMN") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Delimited::new(vec![Ref::new("ComputedColumnDefinitionSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("COLUMN") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Ref::new("ColumnConstraintSegment") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("DROP") .to_matchable()]) .to_matchable(), Ref::new("PeriodSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("NOCHECK") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("ADD") .to_matchable(), Ref::new("TableConstraintSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("NOCHECK") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("NOCHECK") .to_matchable()]) .to_matchable(), Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RENAME") .to_matchable(), one_of(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("TO") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), one_of(vec![Bracketed::new(vec![Sequence::new(vec![Ref::keyword("FILESTREAM_ON") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("FilegroupNameSegment") .to_matchable(), Ref::new("PartitionSchemeNameSegment") .to_matchable(), one_of(vec![Ref::keyword("NULL") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("SYSTEM_VERSIONING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Ref::keyword("HISTORY_TABLE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("DATA_CONSISTENCY_CHECK") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("HISTORY_RETENTION_PERIOD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .optional() .to_matchable(), Ref::new("DatetimeUnitSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::keyword("DATA_DELETION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Ref::keyword("FILTER_COLUMN") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::keyword("RETENTION_PERIOD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .optional() .to_matchable(), Ref::new("DatetimeUnitSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("PrimaryKeyGrammar") .to_matchable(), Ref::new("BracketedIndexColumnListGrammar") .to_matchable(), Ref::new("RelationalIndexOptionsSegment") .optional() .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ForeignKeyGrammar") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable(), Ref::new("ReferencesConstraintGrammar") .to_matchable()]) .to_matchable(), Ref::new("CheckConstraintGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GraphTableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::GraphTableConstraint, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("PrimaryKeyGrammar") .to_matchable(), Ref::new("BracketedIndexColumnListGrammar") .to_matchable(), Ref::new("RelationalIndexOptionsSegment") .optional() .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ForeignKeyGrammar") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable(), Ref::new("ReferencesConstraintGrammar") .to_matchable()]) .to_matchable(), Ref::new("ConnectionConstraintGrammar") .optional() .to_matchable(), Ref::new("CheckConstraintGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableIndexSegment".into(),
            NodeMatcher::new(SyntaxKind::TableIndexSegment, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![Ref::keyword("INDEX") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("UNIQUE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("BracketedIndexColumnListGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("NONCLUSTERED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("RelationalIndexOptionsSegment") .optional() .to_matchable(), Ref::new("OnPartitionOrFilegroupOptionSegment") .optional() .to_matchable(), Ref::new("FilestreamOnOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BracketedIndexColumnListGrammar".into(),
            NodeMatcher::new(SyntaxKind::BracketedIndexColumnListGrammar, |_dialect| {
                Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![Ref::new("IndexColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FilegroupNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FilegroupName, |_dialect| {
                Ref::new("SingleIdentifierGrammar")
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FilegroupClause".into(),
            NodeMatcher::new(SyntaxKind::FilegroupClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::new("FilegroupNameSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateFullTextCatalogStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFulltextCatalogStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("FULLTEXT") .to_matchable(), Ref::keyword("CATALOG") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("FILEGROUP") .to_matchable(), Ref::new("FilegroupNameSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("IN") .to_matchable(), Ref::keyword("PATH") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("ACCENT_SENSITIVITY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("DEFAULT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AUTHORIZATION") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenXmlSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenxmlSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPENXML") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Delimited::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable(), Ref::new("NumericLiteralSegment") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IdentityGrammar".into(),
            NodeMatcher::new(SyntaxKind::IdentityGrammar, |_dialect| {
                Sequence::new(vec![Ref::keyword("IDENTITY") .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "EncryptedWithGrammar".into(),
            NodeMatcher::new(SyntaxKind::EncryptedWithGrammar, |_dialect| {
                Sequence::new(vec![Ref::keyword("ENCRYPTED") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("COLUMN_ENCRYPTION_KEY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENCRYPTION_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("DETERMINISTIC") .to_matchable(), Ref::keyword("RANDOMIZED") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableDistributionIndexClause".into(),
            NodeMatcher::new(SyntaxKind::TableDistributionIndexClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableDistributionClause") .to_matchable(), Ref::new("TableIndexClause") .to_matchable(), Ref::new("TableLocationClause") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableDistributionClause".into(),
            NodeMatcher::new(SyntaxKind::TableDistributionClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("DISTRIBUTION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("REPLICATE") .to_matchable(), Ref::keyword("ROUND_ROBIN") .to_matchable(), Sequence::new(vec![Ref::keyword("HASH") .to_matchable(), Bracketed::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableIndexClause".into(),
            NodeMatcher::new(SyntaxKind::TableIndexClause, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("HEAP") .to_matchable(), Sequence::new(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("COLUMNSTORE") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CLUSTERED") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("ASC") .to_matchable(), Ref::keyword("DESC") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableLocationClause".into(),
            NodeMatcher::new(SyntaxKind::TableLocationClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("LOCATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("USER_DB") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterTableSwitchStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableSwitchStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("SWITCH") .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Bracketed::new(vec![Ref::keyword("WAIT_AT_LOW_PRIORITY") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("MAX_DURATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("MINUTES") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ABORT_AFTER_WAIT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Ref::keyword("SELF") .to_matchable(), Ref::keyword("BLOCKERS") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::keyword("TRUNCATE_TARGET") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTableAsSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableAsSelectStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("TableDistributionIndexClause") .to_matchable(), Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TransactionStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), Sequence::new(vec![Ref::keyword("DISTRIBUTED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TransactionGrammar") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("MARK") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("COMMIT") .to_matchable(), Ref::keyword("ROLLBACK") .to_matchable()]) .to_matchable(), Ref::new("TransactionGrammar") .optional() .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("COMMIT") .to_matchable(), Ref::keyword("ROLLBACK") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WORK") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SAVE") .to_matchable(), Ref::new("TransactionGrammar") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "BeginEndSegment".into(),
            NodeMatcher::new(SyntaxKind::BeginEndBlock, |_dialect| {
                Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("OneOrMoreStatementsGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("END") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AtomicBeginEndSegment".into(),
            NodeMatcher::new(SyntaxKind::AtomicBeginEndBlock, |_dialect| {
                Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), Sequence::new(vec![Ref::keyword("ATOMIC") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TRANSACTION") .to_matchable(), Ref::keyword("ISOLATION") .to_matchable(), Ref::keyword("LEVEL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("SNAPSHOT") .to_matchable(), Sequence::new(vec![Ref::keyword("REPEATABLE") .to_matchable(), Ref::keyword("READ") .to_matchable()]) .to_matchable(), Ref::keyword("SERIALIZABLE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATEFIRST") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("DATEFORMAT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("DateFormatSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("DELAYED_DURABILITY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("OneOrMoreStatementsGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TryCatchSegment".into(),
            NodeMatcher::new(SyntaxKind::TryCatch, |_dialect| {
                Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), Ref::keyword("TRY") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("OneOrMoreStatementsGrammar") .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("END") .to_matchable(), Ref::keyword("TRY") .to_matchable(), Ref::keyword("BEGIN") .to_matchable(), Ref::keyword("CATCH") .to_matchable(), MetaSegment::indent() .to_matchable(), AnyNumberOf::new(vec![Ref::new("StatementAndDelimiterGrammar") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("END") .to_matchable(), Ref::keyword("CATCH") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenRowSetSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenrowsetSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPENROWSET") .to_matchable(), Bracketed::new(vec![one_of(vec![Sequence::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("DelimiterGrammar") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("DelimiterGrammar") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), one_of(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("BULK") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("FORMATFILE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable(), Ref::new("CommaSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("DATA_SOURCE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ERRORFILE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ERRORFILE_DATA_SOURCE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MAXERRORS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIRSTROW") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LASTROW") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CODEPAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FORMAT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIELDQUOTE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FORMATFILE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FORMATFILE_DATA_SOURCE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("SINGLE_BLOB") .to_matchable(), Ref::keyword("SINGLE_CLOB") .to_matchable(), Ref::keyword("SINGLE_NCLOB") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("OpenRowSetWithClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenRowSetWithClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenrowsetWithClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("CollateGrammar") .optional() .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DELETE") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("TopPercentGrammar") .optional() .to_matchable(), Ref::keyword("FROM") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("OPENDATASOURCE") .to_matchable(), Bracketed::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("PostTableExpressionGrammar") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OPENQUERY") .to_matchable(), Bracketed::new(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("OpenRowSetSegment") .to_matchable()]) .to_matchable(), Ref::new("OutputClauseSegment") .optional() .to_matchable(), Ref::new("FromClauseSegment") .optional() .to_matchable(), one_of(vec![Ref::new("WhereClauseSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WHERE") .to_matchable(), Ref::keyword("CURRENT") .to_matchable(), Ref::keyword("OF") .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("JOIN") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("JoinOnConditionSegment") .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable()]) .to_matchable(), Ref::new("OpenQuerySegment") .to_matchable()]) .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FromClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FromClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), Delimited::new(vec![Ref::new("FromExpressionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_dialect| {
                one_of(vec![Ref::new("ValuesClauseSegment") .to_matchable(), Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("PostTableExpressionGrammar") .to_matchable()]) .to_matchable(), Ref::new("BareFunctionSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Ref::new("OpenRowSetSegment") .to_matchable(), Ref::new("OpenJsonSegment") .to_matchable(), Ref::new("OpenXmlSegment") .to_matchable(), Ref::new("OpenQuerySegment") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("StorageLocationSegment") .to_matchable(), Bracketed::new(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("MergeStatementSegment") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("DeleteStatementSegment") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("InsertStatementSegment") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("UpdateStatementSegment") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::new("TableExpressionSegment") .to_matchable(), Conditional::new(MetaSegment::dedent()) .to_matchable(), Conditional::new(MetaSegment::indent()) .indented_joins() .to_matchable(), one_of(vec![Ref::new("JoinClauseSegment") .to_matchable(), Ref::new("JoinLikeClauseGrammar") .to_matchable()]) .to_matchable(), Conditional::new(MetaSegment::dedent()) .indented_joins() .to_matchable(), Conditional::new(MetaSegment::indent()) .indented_joins() .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GroupByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::GroupbyClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("GROUP") .to_matchable(), Ref::keyword("BY") .to_matchable(), MetaSegment::indent() .to_matchable(), one_of(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), one_of(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("WithRollupClauseSegment") .optional() .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WithRollupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithRollupClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("ROLLUP") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "HavingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::HavingClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("HAVING") .to_matchable(), MetaSegment::indent() .to_matchable(), optionally_bracketed(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OrderByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OrderbyClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Ref::keyword("BY") .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ASC") .to_matchable(), Ref::keyword("DESC") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.terminators = vec![Ref::new("OffsetClauseSegment") .to_matchable()]; }) .to_matchable(), Sequence::new(vec![Ref::new("OffsetClauseSegment") .to_matchable(), Ref::new("FetchClauseSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OffsetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OffsetClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("OFFSET") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ROW") .to_matchable(), Ref::keyword("ROWS") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RenameStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RenameStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RENAME") .to_matchable(), Ref::keyword("OBJECT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("UPDATE") .to_matchable(), MetaSegment::indent() .to_matchable(), one_of(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("AliasedTableReferenceGrammar") .to_matchable(), Ref::new("OpenQuerySegment") .to_matchable()]) .to_matchable(), Ref::new("PostTableExpressionGrammar") .optional() .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::new("SetClauseListSegment") .to_matchable(), Ref::new("OutputClauseSegment") .optional() .to_matchable(), Ref::new("FromClauseSegment") .optional() .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetClauseListSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClauseList, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("SetClauseSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("SetClauseSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClause, |_dialect| {
                Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("AssignmentOperatorSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetContextInfoSegment".into(),
            NodeMatcher::new(SyntaxKind::SetContextInfoStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("CONTEXT_INFO") .to_matchable(), one_of(vec![Ref::new("HexadecimalLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetLanguageStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetLanguageStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("LANGUAGE") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("BracketedIdentifierSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PrintStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PrintStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("PRINT") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OptionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OptionClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPTION") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("QueryHintSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "QueryHintSegment".into(),
            NodeMatcher::new(SyntaxKind::QueryHintSegment, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("LABEL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("HASH") .to_matchable(), Ref::keyword("ORDER") .to_matchable()]) .to_matchable(), Ref::keyword("GROUP") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("MERGE") .to_matchable(), Ref::keyword("HASH") .to_matchable(), Ref::keyword("CONCAT") .to_matchable()]) .to_matchable(), Ref::keyword("UNION") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("LOOP") .to_matchable(), Ref::keyword("MERGE") .to_matchable(), Ref::keyword("HASH") .to_matchable()]) .to_matchable(), Ref::keyword("JOIN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("EXPAND") .to_matchable(), Ref::keyword("VIEWS") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FAST") .to_matchable(), Ref::keyword("MAXDOP") .to_matchable(), Ref::keyword("MAXRECURSION") .to_matchable(), Ref::keyword("QUERYTRACEON") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("MAX_GRANT_PERCENT") .to_matchable(), Ref::keyword("MIN_GRANT_PERCENT") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FORCE") .to_matchable(), Ref::keyword("ORDER") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FORCE") .to_matchable(), Ref::keyword("DISABLE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("EXTERNALPUSHDOWN") .to_matchable(), Ref::keyword("SCALEOUTEXECUTION") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("KEEP") .to_matchable(), Ref::keyword("KEEPFIXED") .to_matchable(), Ref::keyword("ROBUST") .to_matchable()]) .to_matchable(), Ref::keyword("PLAN") .to_matchable()]) .to_matchable(), Ref::keyword("IGNORE_NONCLUSTERED_COLUMNSTORE_INDEX") .to_matchable(), Ref::keyword("NO_PERFORMANCE_SPOOL") .to_matchable(), Sequence::new(vec![Ref::keyword("OPTIMIZE") .to_matchable(), Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::keyword("UNKNOWN") .to_matchable(), Bracketed::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), one_of(vec![Ref::keyword("UNKNOWN") .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), one_of(vec![Ref::keyword("UNKNOWN") .to_matchable(), Sequence::new(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PARAMETERIZATION") .to_matchable(), one_of(vec![Ref::keyword("SIMPLE") .to_matchable(), Ref::keyword("FORCED") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("RECOMPILE") .to_matchable(), Sequence::new(vec![Ref::keyword("USE") .to_matchable(), Ref::keyword("HINT") .to_matchable(), Bracketed::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("USE") .to_matchable(), Ref::keyword("PLAN") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TABLE") .to_matchable(), Ref::keyword("HINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Delimited::new(vec![Ref::new("TableHintSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PostTableExpressionGrammar".into(),
            NodeMatcher::new(SyntaxKind::PostTableExpression, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![Ref::keyword("WITH") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Bracketed::new(vec![Ref::new("TableHintSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("TableHintSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableHintSegment".into(),
            NodeMatcher::new(SyntaxKind::QueryHintSegment, |_dialect| {
                one_of(vec![Ref::keyword("NOEXPAND") .to_matchable(), Sequence::new(vec![Ref::keyword("INDEX") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("IndexReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("INDEX") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Bracketed::new(vec![one_of(vec![Ref::new("IndexReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("KEEPIDENTITY") .to_matchable(), Ref::keyword("KEEPDEFAULTS") .to_matchable(), Sequence::new(vec![Ref::keyword("FORCESEEK") .to_matchable(), Bracketed::new(vec![Ref::new("IndexReferenceSegment") .to_matchable(), Bracketed::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("FORCESCAN") .to_matchable(), Ref::keyword("HOLDLOCK") .to_matchable(), Ref::keyword("IGNORE_CONSTRAINTS") .to_matchable(), Ref::keyword("IGNORE_TRIGGERS") .to_matchable(), Ref::keyword("NOLOCK") .to_matchable(), Ref::keyword("NOWAIT") .to_matchable(), Ref::keyword("PAGLOCK") .to_matchable(), Ref::keyword("READCOMMITTED") .to_matchable(), Ref::keyword("READCOMMITTEDLOCK") .to_matchable(), Ref::keyword("READPAST") .to_matchable(), Ref::keyword("READUNCOMMITTED") .to_matchable(), Ref::keyword("REPEATABLEREAD") .to_matchable(), Ref::keyword("ROWLOCK") .to_matchable(), Ref::keyword("SERIALIZABLE") .to_matchable(), Ref::keyword("SNAPSHOT") .to_matchable(), Sequence::new(vec![Ref::keyword("SPATIAL_WINDOW_MAX_CELLS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Ref::keyword("TABLOCK") .to_matchable(), Ref::keyword("TABLOCKX") .to_matchable(), Ref::keyword("UPDLOCK") .to_matchable(), Ref::keyword("XLOCK") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("UNION") .to_matchable(), one_of(vec![Ref::keyword("DISTINCT") .to_matchable(), Ref::keyword("ALL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("INTERSECT") .to_matchable(), Ref::keyword("EXCEPT") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SetExpression, |_dialect| {
                Sequence::new(vec![Ref::new("NonSetSelectableGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::new("SetOperatorSegment") .to_matchable(), Ref::new("NonSetSelectableGrammar") .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(1); }) .to_matchable(), Ref::new("OrderByClauseSegment") .optional() .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ForClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ForClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::keyword("BROWSE") .to_matchable(), Sequence::new(vec![Ref::keyword("JSON") .to_matchable(), Delimited::new(vec![one_of(vec![Ref::keyword("AUTO") .to_matchable(), Ref::keyword("PATH") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ROOT") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("INCLUDE_NULL_VALUES") .optional() .to_matchable(), Ref::keyword("WITHOUT_ARRAY_WRAPPER") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("XML") .to_matchable(), one_of(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("PATH") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("BASE64") .to_matchable()]) .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("ROOT") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ELEMENTS") .to_matchable(), one_of(vec![Ref::keyword("XSINIL") .to_matchable(), Ref::keyword("ABSENT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Delimited::new(vec![Ref::keyword("EXPLICIT") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("BASE64") .to_matchable()]) .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("ROOT") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("XMLDATA") .optional() .to_matchable()]) .to_matchable(), Delimited::new(vec![one_of(vec![Ref::keyword("AUTO") .to_matchable(), Sequence::new(vec![Ref::keyword("RAW") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("BASE64") .to_matchable()]) .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("ROOT") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ELEMENTS") .to_matchable(), one_of(vec![Ref::keyword("XSINIL") .to_matchable(), Ref::keyword("ABSENT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("XMLDATA") .to_matchable(), Sequence::new(vec![Ref::keyword("XMLSCHEMA") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExecuteOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteOption, |_dialect| {
                one_of(vec![Ref::keyword("RECOMPILE") .to_matchable(), Sequence::new(vec![Ref::keyword("RESULT") .to_matchable(), Ref::keyword("SETS") .to_matchable(), Ref::keyword("UNDEFINED") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RESULT") .to_matchable(), Ref::keyword("SETS") .to_matchable(), Ref::keyword("NONE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RESULT") .to_matchable(), Ref::keyword("SETS") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("NULL") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("OBJECT") .to_matchable(), Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .optional() .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("XML") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LoginUserSegment".into(),
            NodeMatcher::new(SyntaxKind::LoginUserSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("AS") .to_matchable(), one_of(vec![Ref::keyword("LOGIN") .to_matchable(), Ref::keyword("USER") .to_matchable()]) .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExecuteScriptSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteScriptStatement, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("EXEC") .to_matchable(), Ref::keyword("EXECUTE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("SemicolonSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), MetaSegment::indent() .to_matchable(), AnyNumberOf::new(vec![Delimited::new(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("EqualsSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("OUTPUT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("DEFAULT") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::new("ExecuteOptionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()]) .config(|this| { this.delimiter(Ref::new("PlusSegment")); }) .to_matchable()]) .to_matchable(), Ref::new("LoginUserSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()]) .config(|this| { this.delimiter(Ref::new("PlusSegment")); }) .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OUTPUT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("LoginUserSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("AT") .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_SOURCE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateSchemaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSchemaStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::new("SchemaReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("AUTHORIZATION") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MergeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeStatement, |_dialect| {
                Sequence::new(vec![Ref::new("MergeIntoLiteralGrammar") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableHintSegment") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("AliasExpressionSegment") .exclude(Ref::keyword("USING")) .optional() .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("USING") .to_matchable(), MetaSegment::indent() .to_matchable(), one_of(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("AliasedTableReferenceGrammar") .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Ref::new("AliasExpressionSegment") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable(), Conditional::new(MetaSegment::indent()) .indented_using_on() .to_matchable(), Ref::new("JoinOnConditionSegment") .to_matchable(), Conditional::new(MetaSegment::dedent()) .indented_using_on() .to_matchable(), Ref::new("MergeMatchSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeMatch, |_dialect| {
                Sequence::new(vec![AnyNumberOf::new(vec![Ref::new("MergeMatchedClauseSegment") .to_matchable(), Ref::new("MergeNotMatchedClauseSegment") .to_matchable()]) .config(|this| { this.min_times(1); }) .to_matchable(), Ref::new("OutputClauseSegment") .optional() .to_matchable(), Ref::new("OptionClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MergeMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenMatchedClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WHEN") .to_matchable(), Ref::keyword("MATCHED") .to_matchable(), Sequence::new(vec![Ref::keyword("AND") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::keyword("THEN") .to_matchable(), one_of(vec![Ref::new("MergeUpdateClauseSegment") .to_matchable(), Ref::new("MergeDeleteClauseSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MergeNotMatchedClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenNotMatchedClause, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("WHEN") .to_matchable(), Ref::keyword("NOT") .to_matchable(), Ref::keyword("MATCHED") .to_matchable(), Sequence::new(vec![Ref::keyword("BY") .to_matchable(), Ref::keyword("TARGET") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AND") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::keyword("THEN") .to_matchable(), Ref::new("MergeInsertClauseSegment") .to_matchable(), MetaSegment::dedent() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WHEN") .to_matchable(), Ref::keyword("NOT") .to_matchable(), Ref::keyword("MATCHED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::keyword("SOURCE") .to_matchable(), Sequence::new(vec![Ref::keyword("AND") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::keyword("THEN") .to_matchable(), one_of(vec![Ref::new("MergeUpdateClauseSegment") .to_matchable(), Ref::new("MergeDeleteClauseSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeInsertClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("INSERT") .to_matchable(), MetaSegment::indent() .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::keyword("VALUES") .to_matchable(), MetaSegment::indent() .to_matchable(), one_of(vec![Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("VALUES") .to_matchable()]) .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OutputClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OutputClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("OUTPUT") .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("DELETED") .to_matchable(), Ref::keyword("INSERTED") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable(), Ref::new("DotSegment") .to_matchable(), one_of(vec![Ref::new("WildcardIdentifierSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("ActionParameterSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Ref::new("AliasExpressionSegment") .optional() .to_matchable()]) .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("INTO") .to_matchable(), Ref::keyword("FROM") .to_matchable()]; }) .to_matchable(), MetaSegment::dedent() .to_matchable(), Sequence::new(vec![Ref::keyword("INTO") .to_matchable(), MetaSegment::indent() .to_matchable(), one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), MetaSegment::dedent() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ThrowStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ThrowStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("THROW") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentWithN") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Ref::new("CommaSegment") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RaiserrorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RaiserrorStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RAISERROR") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegmentWithN") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("CommaSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Ref::new("CommaSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![Ref::keyword("LOG") .to_matchable(), Ref::keyword("NOWAIT") .to_matchable(), Ref::keyword("SETERROR") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GotoStatement".into(),
            NodeMatcher::new(SyntaxKind::GotoStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("GOTO") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExecuteAsClause".into(),
            NodeMatcher::new(SyntaxKind::ExecuteAsClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("EXECUTE") .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Sequence::new(vec![Ref::keyword("OR") .to_matchable(), Ref::keyword("ALTER") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TRIGGER") .to_matchable(), Ref::new("TriggerReferenceSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SERVER") .to_matchable()]) .to_matchable(), Ref::keyword("DATABASE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), AnyNumberOf::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("NATIVE_COMPILATION") .to_matchable(), Ref::keyword("SCHEMABINDING") .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable(), Ref::new("ExecuteAsClause") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Delimited::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::keyword("AFTER") .to_matchable(), Sequence::new(vec![Ref::keyword("INSTEAD") .to_matchable(), Ref::keyword("OF") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::keyword("INSERT") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("DELETE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("APPEND") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("OneOrMoreStatementsGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("TRIGGER") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Delimited::new(vec![Ref::new("TriggerReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SERVER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DisableTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DisableTrigger, |_dialect| {
                Sequence::new(vec![Ref::keyword("DISABLE") .to_matchable(), Ref::keyword("TRIGGER") .to_matchable(), one_of(vec![Delimited::new(vec![Ref::new("TriggerReferenceSegment") .to_matchable()]) .to_matchable(), Ref::keyword("ALL") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SERVER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LabelStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::LabelSegment, |_dialect| {
                Sequence::new(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("ColonSegment") .to_matchable()])
                    .config(|this| {
                        this.disallow_gaps();
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AccessStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AccessStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("GRANT") .to_matchable(), one_of(vec![Sequence::new(vec![Delimited::new(vec![one_of(vec![one_of(vec![Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("ROLE") .to_matchable(), Ref::keyword("USER") .to_matchable(), Ref::keyword("WAREHOUSE") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("INTEGRATION") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("APPLY") .to_matchable(), Ref::keyword("MASKING") .to_matchable(), Ref::keyword("POLICY") .to_matchable()]) .to_matchable(), Ref::keyword("EXECUTE") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("CONTROL") .to_matchable(), Ref::keyword("DELETE") .to_matchable(), Ref::keyword("EXECUTE") .to_matchable(), Ref::keyword("INSERT") .to_matchable(), Ref::keyword("RECEIVE") .to_matchable(), Ref::keyword("REFERENCES") .to_matchable(), Ref::keyword("SELECT") .to_matchable(), Sequence::new(vec![Ref::keyword("TAKE") .to_matchable(), Ref::keyword("OWNERSHIP") .to_matchable()]) .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("CHANGE") .to_matchable(), Ref::keyword("TRACKING") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("DEFINITION") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("ON") .to_matchable()]; }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("PRIVILEGES") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("LOGIN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("OBJECT") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("USER") .to_matchable()]) .to_matchable(), Ref::new("CastOperatorSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("LANGUAGE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("FOREIGN") .to_matchable(), one_of(vec![Ref::keyword("SERVER") .to_matchable(), Sequence::new(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("WRAPPER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SCHEMAS") .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TABLE") .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::keyword("PROCEDURE") .to_matchable(), Ref::keyword("SEQUENCE") .to_matchable(), Sequence::new(vec![Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("TABLE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILE") .to_matchable(), Ref::keyword("FORMAT") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), one_of(vec![Ref::keyword("TABLES") .to_matchable(), Ref::keyword("VIEWS") .to_matchable(), Ref::keyword("FUNCTIONS") .to_matchable(), Ref::keyword("PROCEDURES") .to_matchable(), Ref::keyword("SEQUENCES") .to_matchable()]) .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("TO") .to_matchable(), Ref::keyword("FROM") .to_matchable()]; }) .to_matchable(), Ref::new("FunctionParameterListGrammar") .optional() .to_matchable()]) .to_matchable(), Ref::keyword("TO") .to_matchable(), Delimited::new(vec![one_of(vec![Ref::new("RoleReferenceSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("GRANT") .to_matchable(), Ref::keyword("OPTION") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DENY") .to_matchable(), one_of(vec![Delimited::new(vec![one_of(vec![one_of(vec![Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("ROLE") .to_matchable(), Ref::keyword("USER") .to_matchable(), Ref::keyword("WAREHOUSE") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("INTEGRATION") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("APPLY") .to_matchable(), Ref::keyword("MASKING") .to_matchable(), Ref::keyword("POLICY") .to_matchable()]) .to_matchable(), Ref::keyword("EXECUTE") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("CONTROL") .to_matchable(), Ref::keyword("DELETE") .to_matchable(), Ref::keyword("EXECUTE") .to_matchable(), Ref::keyword("INSERT") .to_matchable(), Ref::keyword("RECEIVE") .to_matchable(), Ref::keyword("REFERENCES") .to_matchable(), Ref::keyword("SELECT") .to_matchable(), Sequence::new(vec![Ref::keyword("TAKE") .to_matchable(), Ref::keyword("OWNERSHIP") .to_matchable()]) .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("CHANGE") .to_matchable(), Ref::keyword("TRACKING") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("DEFINITION") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("ON") .to_matchable()]; }) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("PRIVILEGES") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("LOGIN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("OBJECT") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("USER") .to_matchable()]) .to_matchable(), Ref::new("CastOperatorSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("LANGUAGE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("FOREIGN") .to_matchable(), one_of(vec![Ref::keyword("SERVER") .to_matchable(), Sequence::new(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("WRAPPER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SCHEMAS") .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TABLE") .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::keyword("PROCEDURE") .to_matchable(), Ref::keyword("SEQUENCE") .to_matchable(), Sequence::new(vec![Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("TABLE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILE") .to_matchable(), Ref::keyword("FORMAT") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), one_of(vec![Ref::keyword("TABLES") .to_matchable(), Ref::keyword("VIEWS") .to_matchable(), Ref::keyword("FUNCTIONS") .to_matchable(), Ref::keyword("PROCEDURES") .to_matchable(), Ref::keyword("SEQUENCES") .to_matchable()]) .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("TO") .to_matchable(), Ref::keyword("FROM") .to_matchable()]; }) .to_matchable(), Ref::new("FunctionParameterListGrammar") .optional() .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TO") .to_matchable()]) .to_matchable(), Delimited::new(vec![Ref::new("RoleReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CASCADE") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REVOKE") .to_matchable(), Sequence::new(vec![Ref::keyword("GRANT") .to_matchable(), Ref::keyword("OPTION") .to_matchable(), Ref::keyword("FOR") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Delimited::new(vec![one_of(vec![one_of(vec![Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("ROLE") .to_matchable(), Ref::keyword("USER") .to_matchable(), Ref::keyword("WAREHOUSE") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("INTEGRATION") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("APPLY") .to_matchable(), Ref::keyword("MASKING") .to_matchable(), Ref::keyword("POLICY") .to_matchable()]) .to_matchable(), Ref::keyword("EXECUTE") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("CONTROL") .to_matchable(), Ref::keyword("DELETE") .to_matchable(), Ref::keyword("EXECUTE") .to_matchable(), Ref::keyword("INSERT") .to_matchable(), Ref::keyword("RECEIVE") .to_matchable(), Ref::keyword("REFERENCES") .to_matchable(), Ref::keyword("SELECT") .to_matchable(), Sequence::new(vec![Ref::keyword("TAKE") .to_matchable(), Ref::keyword("OWNERSHIP") .to_matchable()]) .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("CHANGE") .to_matchable(), Ref::keyword("TRACKING") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("VIEW") .to_matchable(), Ref::keyword("DEFINITION") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("ON") .to_matchable()]; }) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("PRIVILEGES") .optional() .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("LOGIN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("OBJECT") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("USER") .to_matchable()]) .to_matchable(), Ref::new("CastOperatorSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("LANGUAGE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Sequence::new(vec![Ref::keyword("FOREIGN") .to_matchable(), one_of(vec![Ref::keyword("SERVER") .to_matchable(), Sequence::new(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("WRAPPER") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), Ref::keyword("SCHEMAS") .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("DATABASE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TABLE") .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::keyword("PROCEDURE") .to_matchable(), Ref::keyword("SEQUENCE") .to_matchable(), Sequence::new(vec![Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("TABLE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILE") .to_matchable(), Ref::keyword("FORMAT") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALL") .to_matchable(), one_of(vec![Ref::keyword("TABLES") .to_matchable(), Ref::keyword("VIEWS") .to_matchable(), Ref::keyword("FUNCTIONS") .to_matchable(), Ref::keyword("PROCEDURES") .to_matchable(), Ref::keyword("SEQUENCES") .to_matchable()]) .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("TO") .to_matchable(), Ref::keyword("FROM") .to_matchable()]; }) .to_matchable(), Ref::new("FunctionParameterListGrammar") .optional() .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TO") .to_matchable(), Ref::keyword("FROM") .to_matchable()]) .to_matchable(), Delimited::new(vec![Ref::new("RoleReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CASCADE") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTypeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTypeStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("TYPE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("TableConstraintSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), Ref::new("TableIndexSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.allow_trailing(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenCursorStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPEN") .to_matchable(), Ref::keyword("GLOBAL") .optional() .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CloseCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CloseCursorStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CLOSE") .to_matchable(), Ref::keyword("GLOBAL") .optional() .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeallocateCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeallocateCursorStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DEALLOCATE") .to_matchable(), Ref::keyword("GLOBAL") .optional() .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FetchCursorStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FetchCursorStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("FETCH") .to_matchable(), one_of(vec![Ref::keyword("NEXT") .to_matchable(), Ref::keyword("PRIOR") .to_matchable(), Ref::keyword("FIRST") .to_matchable(), Ref::keyword("LAST") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ABSOLUTE") .to_matchable(), Ref::keyword("RELATIVE") .to_matchable()]) .to_matchable(), Ref::new("SignedSegmentGrammar") .optional() .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("FROM") .optional() .to_matchable(), Ref::new("CursorNameGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("INTO") .to_matchable(), Delimited::new(vec![Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ConcatSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Ref::new("PlusSegment")
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateSynonymStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSynonymStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("SYNONYM") .to_matchable(), Ref::new("SynonymReferenceSegment") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropSynonymStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSynonymStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("SYNONYM") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("SynonymReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SynonymReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::SynonymReference, |_dialect| {
                Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::new("DotSegment") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()]) .config(|this| { this.min_times(0); this.max_times(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_dialect| {
                Sequence::new(vec![Ref::keyword("TABLESAMPLE") .to_matchable(), Sequence::new(vec![Ref::keyword("SYSTEM") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Bracketed::new(vec![Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("PERCENT") .to_matchable(), Ref::keyword("ROWS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("REPEATABLE") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TemporalQuerySegment".into(),
            NodeMatcher::new(SyntaxKind::TemporalQuery, |_dialect| {
                Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("SYSTEM_TIME") .to_matchable(), one_of(vec![Ref::keyword("ALL") .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("OF") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Ref::keyword("TO") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("BETWEEN") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable(), Ref::keyword("AND") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CONTAINED") .to_matchable(), Ref::keyword("IN") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateDatabaseScopedCredentialStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseScopedCredentialStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("SCOPED") .to_matchable(), Ref::keyword("CREDENTIAL") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Ref::new("CredentialGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateExternalDataSourceStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateExternalDataSourceStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("DATA") .to_matchable(), Ref::keyword("SOURCE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableLocationClause") .to_matchable(), Sequence::new(vec![Ref::keyword("CONNECTION_OPTIONS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CREDENTIAL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PUSHDOWN") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PeriodSegment".into(),
            NodeMatcher::new(SyntaxKind::PeriodSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("PERIOD") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("SYSTEM_TIME") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SqlcmdCommandSegment".into(),
            NodeMatcher::new(SyntaxKind::SqlcmdCommandSegment, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("ColonSegment") .to_matchable(), Ref::new("SqlcmdOperatorSegment") .to_matchable()]) .config(|this| { this.disallow_gaps(); }) .to_matchable(), Ref::new("SqlcmdFilePathSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::new("ColonSegment") .to_matchable(), Ref::new("SqlcmdOperatorSegment") .to_matchable()]) .config(|this| { this.disallow_gaps(); }) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::new("CodeSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatDelimitedTextFormatOptionClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileDelimitedTextFormatOptionsClause, |_dialect| {
                one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("FIELD_TERMINATOR") .to_matchable(), Ref::keyword("STRING_DELIMITER") .to_matchable(), Ref::keyword("DATE_FORMAT") .to_matchable(), Ref::keyword("PARSER_VERSION") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIRST_ROW") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("USE_TYPE_DEFAULT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("BooleanLiteralGrammar") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENCODING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileEncodingSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatDelimitedTextClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileDelimitedTextClause, |_dialect| {
                Delimited::new(vec![Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("DELIMITEDTEXT") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FORMAT_OPTIONS") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ExternalFileFormatDelimitedTextFormatOptionClause") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileCompressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatRcClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileRcClause, |_dialect| {
                Delimited::new(vec![Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("RCFILE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SERDE_METHOD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("SerdeMethodSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileCompressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatOrcClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileOrcClause, |_dialect| {
                Delimited::new(vec![Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("ORC") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileCompressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatParquetClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileParquetClause, |_dialect| {
                Delimited::new(vec![Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("PARQUET") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileCompressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatJsonClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileJsonClause, |_dialect| {
                Delimited::new(vec![Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("JSON") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileCompressionSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExternalFileFormatDeltaClause".into(),
            NodeMatcher::new(SyntaxKind::ExternalFileDeltaClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("FORMAT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::keyword("DELTA") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateExternalFileFormat".into(),
            NodeMatcher::new(SyntaxKind::CreateExternalFileFormat, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("FILE") .to_matchable(), Ref::keyword("FORMAT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![one_of(vec![Ref::new("ExternalFileFormatDelimitedTextClause") .to_matchable(), Ref::new("ExternalFileFormatRcClause") .to_matchable(), Ref::new("ExternalFileFormatOrcClause") .to_matchable(), Ref::new("ExternalFileFormatParquetClause") .to_matchable(), Ref::new("ExternalFileFormatJsonClause") .to_matchable(), Ref::new("ExternalFileFormatDeltaClause") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenJsonWithClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenjsonWithClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("JSON") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenJsonSegment".into(),
            NodeMatcher::new(SyntaxKind::OpenjsonSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPENJSON") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("QuotedLiteralSegmentOptWithN") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("OpenJsonWithClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenQuerySegment".into(),
            NodeMatcher::new(SyntaxKind::OpenquerySegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPENQUERY") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateExternalTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateExternalTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableLocationClause") .to_matchable(), Sequence::new(vec![Ref::keyword("DATA_SOURCE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILE_FORMAT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REJECT_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("VALUE") .to_matchable(), Ref::keyword("PERCENTAGE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REJECT_VALUE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REJECT_SAMPLE_VALUE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REJECTED_ROW_LOCATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateRoleStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("AUTHORIZATION") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateServerRoleStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateServerRoleStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("SERVER") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("AUTHORIZATION") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateLoginStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateLoginStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("LOGIN") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), one_of(vec![Ref::keyword("WINDOWS") .to_matchable(), Sequence::new(vec![Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("PROVIDER") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CERTIFICATE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("ASYMMETRIC") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::keyword("MUST_CHANGE") .optional() .to_matchable(), Ref::new("CommaSegment") .optional() .to_matchable(), Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("SID") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("HexadecimalLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_DATABASE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CHECK_EXPIRATION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CHECK_POLICY") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CREDENTIAL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropExternalTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropExternalTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "StorageLocationSegment".into(),
            NodeMatcher::new(SyntaxKind::StorageLocation, |_dialect| {
                one_of(vec![Ref::new("AzureBlobStoragePath") .to_matchable(), Ref::new("AzureDataLakeStorageGen2Path") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CopyIntoTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CopyIntoTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("COPY") .to_matchable(), Ref::keyword("INTO") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("FromClauseSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("FILE_TYPE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FILE_FORMAT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CREDENTIAL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Bracketed::new(vec![Ref::new("CredentialGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ERRORFILE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ERRORFILE_CREDENTIAL") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Bracketed::new(vec![Ref::new("CredentialGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MAXERRORS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COMPRESSION") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIELDQUOTE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIELDTERMINATOR") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ROWTERMINATOR") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FIRSTROW") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DATEFORMAT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENCODING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("FileEncodingSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("IDENTITY_INSERT") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AUTO_CREATE_TABLE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateUserStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("USER") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("DEFAULT_SCHEMA") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SID") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("HexadecimalLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALLOW_ENCRYPTED_VALUE_MODIFICATIONS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FROM") .to_matchable(), Ref::keyword("FOR") .to_matchable()]) .to_matchable(), Ref::keyword("LOGIN") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("DEFAULT_SCHEMA") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALLOW_ENCRYPTED_VALUE_MODIFICATIONS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FROM") .to_matchable(), Ref::keyword("FOR") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("CERTIFICATE") .to_matchable(), Sequence::new(vec![Ref::keyword("ASYMMETRIC") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITHOUT") .to_matchable(), Ref::keyword("LOGIN") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("DEFAULT_SCHEMA") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT_LANGUAGE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALLOW_ENCRYPTED_VALUE_MODIFICATIONS") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), Ref::keyword("EXTERNAL") .to_matchable(), Ref::keyword("PROVIDER") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("OBJECT_ID") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ComputedColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::ComputedColumnDefinition, |_dialect| {
                Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![one_of(vec![Ref::new("FunctionSegment") .to_matchable(), Ref::new("BareFunctionSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PERSISTED") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreatePartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::CreatePartitionFunctionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::keyword("RANGE") .to_matchable(), one_of(vec![Ref::keyword("LEFT") .to_matchable(), Ref::keyword("RIGHT") .to_matchable()]) .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("VALUES") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("HexadecimalLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterPartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterPartitionFunctionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("SPLIT") .to_matchable(), Ref::keyword("RANGE") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MERGE") .to_matchable(), Ref::keyword("RANGE") .to_matchable(), Bracketed::new(vec![Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreatePartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::CreatePartitionSchemeStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("SCHEME") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::keyword("PARTITION") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("ALL") .optional() .to_matchable(), Ref::keyword("TO") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("PRIMARY") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterPartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterPartitionSchemeStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("SCHEME") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("NEXT") .to_matchable(), Ref::keyword("USED") .to_matchable(), Ref::new("ObjectReferenceSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::CreateMasterKeyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("MASTER") .to_matchable(), Ref::keyword("KEY") .to_matchable(), Sequence::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MasterKeyEncryptionSegment".into(),
            NodeMatcher::new(SyntaxKind::MasterKeyEncryptionOption, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("SERVICE") .to_matchable(), Ref::keyword("MASTER") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::AlterMasterKeyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("MASTER") .to_matchable(), Ref::keyword("KEY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("FORCE") .optional() .to_matchable(), Ref::keyword("REGENERATE") .to_matchable(), Ref::keyword("WITH") .to_matchable(), Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("MasterKeyEncryptionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("DROP") .to_matchable()]) .to_matchable(), Ref::keyword("ENCRYPTION") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("MasterKeyEncryptionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropMasterKeySegment".into(),
            NodeMatcher::new(SyntaxKind::DropMasterKeyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("MASTER") .to_matchable(), Ref::keyword("KEY") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::CreateSecurityPolicyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), Ref::keyword("POLICY") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), one_of(vec![Ref::keyword("FILTER") .to_matchable(), Ref::keyword("BLOCK") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("PREDICATE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("AFTER") .to_matchable(), one_of(vec![Ref::keyword("INSERT") .to_matchable(), Ref::keyword("UPDATE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("BEFORE") .to_matchable(), one_of(vec![Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("DELETE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("STATE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SCHEMABINDING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::AlterSecurityPolicyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), Ref::keyword("POLICY") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("ALTER") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("FILTER") .to_matchable(), Ref::keyword("BLOCK") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("PREDICATE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("AFTER") .to_matchable(), one_of(vec![Ref::keyword("INSERT") .to_matchable(), Ref::keyword("UPDATE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("BEFORE") .to_matchable(), one_of(vec![Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("DELETE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), one_of(vec![Ref::keyword("FILTER") .to_matchable(), Ref::keyword("BLOCK") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("PREDICATE") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("STATE") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SCHEMABINDING") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::keyword("REPLICATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropSecurityPolicySegment".into(),
            NodeMatcher::new(SyntaxKind::DropSecurityPolicy, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), Ref::keyword("POLICY") .to_matchable(), Sequence::new(vec![Ref::keyword("IF") .to_matchable(), Ref::keyword("EXISTS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OpenSymmetricKeySegment".into(),
            NodeMatcher::new(SyntaxKind::OpenSymmetricKeyStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPEN") .to_matchable(), Ref::keyword("SYMMETRIC") .to_matchable(), Ref::keyword("KEY") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("DECRYPTION") .to_matchable(), Ref::keyword("BY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("CERTIFICATE") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ASYMMETRIC") .to_matchable(), Ref::keyword("KEY") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SYMMETRIC") .to_matchable(), Ref::keyword("KEY") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::Expression, |_dialect| {
                one_of(vec![Ref::new("Expression_A_Grammar") .to_matchable(), Ref::new("NextValueSequenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AdditionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Sequence::new(vec![Ref::new("PlusComparisonSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SubtractionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Sequence::new(vec![Ref::new("MinusComparisonSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "MultiplicationAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Sequence::new(vec![Ref::new("MultiplyComparisonSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DivisionAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Sequence::new(vec![Ref::new("DivideComparisonSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ModulusAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_dialect| {
                Sequence::new(vec![Ref::new("ModuloComparisonSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
    ]);
    
    tsql_dialect
}

pub fn get_unbracketed_file_spec_segment_grammar() -> Matchable {
    Sequence::new(vec![
        Ref::new("LogicalFileNameSegment").optional().to_matchable(),
        Ref::new("FileSpecFileNameSegment").to_matchable(),
        Ref::new("FileSpecSizeSegment").optional().to_matchable(),
        Ref::new("FileSpecMaxSizeSegment").optional().to_matchable(),
        Ref::new("FileSpecFileGrowthSegment")
            .optional()
            .to_matchable(),
    ])
    .to_matchable()
}

pub fn get_unordered_select_statement_segment_grammar() -> Matchable {
    Sequence::new(vec![
        Ref::new("SelectClauseSegment").to_matchable(),
        Ref::new("IntoTableSegment").optional().to_matchable(),
        Ref::new("FromClauseSegment").optional().to_matchable(),
        Ref::new("WhereClauseSegment").optional().to_matchable(),
        Ref::new("GroupByClauseSegment").optional().to_matchable(),
        Ref::new("HavingClauseSegment").optional().to_matchable(),
        Ref::new("NamedWindowSegment").optional().to_matchable(),
    ])
    .to_matchable()
}
