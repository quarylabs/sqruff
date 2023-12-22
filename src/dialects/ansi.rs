use std::collections::HashSet;
use std::vec;

use itertools::Itertools;

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::Ref;
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::{Matcher, RegexLexer, StringLexer};
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, NewlineSegment,
    NewlineSegmentNewArgs, Segment, SegmentConstructorFn, SymbolSegment, SymbolSegmentNewArgs,
    WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Boxed, Config, ToMatchable};

macro_rules! from_segments {
    ($self:expr, $segments:expr) => {{
        let mut new_object = $self.clone();
        new_object.segments = $segments;
        new_object.to_matchable()
    }};
}

pub fn ansi_dialect() -> Dialect {
    let mut ansi_dialect = Dialect::new("FileSegment");

    ansi_dialect.set_lexer_matchers(lexer_matchers());

    // Set the bare functions
    ansi_dialect.sets_mut("bare_functions").extend([
        "current_timestamp",
        "current_time",
        "current_date",
    ]);

    // Set the datetime units
    ansi_dialect.sets_mut("datetime_units").extend([
        "DAY",
        "DAYOFYEAR",
        "HOUR",
        "MILLISECOND",
        "MINUTE",
        "MONTH",
        "QUARTER",
        "SECOND",
        "WEEK",
        "WEEKDAY",
        "YEAR",
    ]);

    ansi_dialect.sets_mut("date_part_function_name").extend(["DATEADD"]);

    // Set Keywords
    ansi_dialect
        .update_keywords_set_from_multiline_string("unreserved_keywords", ANSI_UNRESERVED_KEYWORDS);
    ansi_dialect
        .update_keywords_set_from_multiline_string("reserved_keywords", ANSI_RESERVED_KEYWORDS);

    // Bracket pairs (a set of tuples).
    // (name, startref, endref, persists)
    // NOTE: The `persists` value controls whether this type
    // of bracket is persisted during matching to speed up other
    // parts of the matching process. Round brackets are the most
    // common and match the largest areas and so are sufficient.
    ansi_dialect.update_bracket_sets(
        "bracket_pairs",
        vec![
            ("round".into(), "StartBracketSegment".into(), "EndBracketSegment".into(), true),
            (
                "square".into(),
                "StartSquareBracketSegment".into(),
                "EndSquareBracketSegment".into(),
                false,
            ),
            (
                "curly".into(),
                "StartCurlyBracketSegment".into(),
                "EndCurlyBracketSegment".into(),
                false,
            ),
        ],
    );

    // Set the value table functions. These are functions that, if they appear as
    // an item in "FROM", are treated as returning a COLUMN, not a TABLE.
    // Apparently, among dialects supported by SQLFluff, only BigQuery has this
    // concept, but this set is defined in the ANSI dialect because:
    // - It impacts core linter rules (see AL04 and several other rules that
    //   subclass from it) and how they interpret the contents of table_expressions
    // - At least one other database (DB2) has the same value table function,
    //   UNNEST(), as BigQuery. DB2 is not currently supported by SQLFluff.
    ansi_dialect.sets_mut("value_table_functions");

    let symbol_factory = |segment: &dyn Segment| {
        SymbolSegment::new(
            &segment.get_raw().unwrap(),
            &segment.get_position_marker().unwrap(),
            SymbolSegmentNewArgs {},
        )
    };

    ansi_dialect.extend([
        (
            "FunctionNameIdentifierSegment".into(),
            TypedParser::new("word", |_| unimplemented!(), None, false, None).to_matchable().into(),
        ),
        (
            "NumericLiteralSegment".into(),
            TypedParser::new("numeric_literal", symbol_factory, None, false, None)
                .to_matchable()
                .into(),
        ),
        ("DelimiterGrammar".into(), Ref::new("SemicolonSegment").to_matchable().into()),
        (
            "SemicolonSegment".into(),
            StringParser::new(";", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StartBracketSegment".into(),
            StringParser::new("(", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "EndBracketSegment".into(),
            StringParser::new(")", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StartSquareBracketSegment".into(),
            StringParser::new("[", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "EndSquareBracketSegment".into(),
            StringParser::new("]", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StartCurlyBracketSegment".into(),
            StringParser::new("{", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "EndCurlyBracketSegment".into(),
            StringParser::new("}", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "CommaSegment".into(),
            StringParser::new(",", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "CastOperatorSegment".into(),
            StringParser::new("::", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "StarSegment".into(),
            StringParser::new("*", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "PositiveSegment".into(),
            StringParser::new("+", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "NegativeSegment".into(),
            StringParser::new("-", symbol_factory, None, false, None).to_matchable().into(),
        ),
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                RegexParser::new(
                    "[A-Z0-9_]*[A-Z][A-Z0-9_]*",
                    |segment| {
                        Box::new(KeywordSegment::new(
                            segment.get_raw().unwrap(),
                            segment.get_position_marker().unwrap(),
                        ))
                    },
                    None,
                    false,
                    anti_template.into(),
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "DatatypeIdentifierSegment".into(),
            SegmentGenerator::new(|_| {
                let anti_template = format!("^({})$", "NOT");

                one_of(vec![
                    RegexParser::new(
                        "[A-Z_][A-Z0-9_]*",
                        |segment| {
                            CodeSegment::new(
                                &segment.get_raw().unwrap(),
                                &segment.get_position_marker().unwrap(),
                                CodeSegmentNewArgs::default(),
                            )
                        },
                        None,
                        false,
                        anti_template.into(),
                        None,
                    )
                    .boxed(),
                    Ref::new("SingleIdentifierGrammar")
                        .exclude(Ref::new("NakedIdentifierSegment"))
                        .boxed(),
                ])
                .boxed()
            })
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            SegmentGenerator::new(|_dialect| {
                let pattern = r#"\"?[A-Z][A-Z0-9_]*\"?"#;

                RegexParser::new(pattern, |_| todo!(), None, false, None, None).boxed()
            })
            .into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec![Ref::keyword("FROM").boxed()]).to_matchable().into(),
        ),
        (
            "BareFunctionSegment".into(),
            SegmentGenerator::new(|dialect| {
                MultiStringParser::new(
                    dialect.sets("bare_functions").into_iter().map(Into::into).collect_vec(),
                    |segment| {
                        CodeSegment::new(
                            &segment.get_raw().unwrap(),
                            &segment.get_position_marker().unwrap(),
                            CodeSegmentNewArgs::default(),
                        )
                    },
                    None,
                    false,
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec![Ref::new("NakedIdentifierSegment").boxed()]).to_matchable().into(),
        ),
        ("TableReferenceSegment".into(), Ref::new("SingleIdentifierGrammar").to_matchable().into()),
        (
            "LiteralGrammar".into(),
            one_of(vec![
                Ref::new("NumericLiteralSegment").boxed(),
                Ref::new("NullLiteralSegment").boxed(),
                Ref::new("QualifiedNumericLiteralSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NullLiteralSegment".into(),
            StringParser::new(
                "null",
                |segment| {
                    KeywordSegment::new(
                        segment.get_raw().unwrap(),
                        segment.get_position_marker().unwrap(),
                    )
                    .boxed()
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
    ]);

    ansi_dialect.extend([
        (
            "QualifiedNumericLiteralSegment".into(),
            QualifiedNumericLiteralSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        ("FunctionSegment".into(), FunctionSegment { segments: Vec::new() }.to_matchable().into()),
        (
            "FunctionNameSegment".into(),
            FunctionNameSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "StatementSegment".into(),
            StatementSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "SelectClauseSegment".into(),
            SelectClauseSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        ("SetExpressionSegment".into(), SetExpressionSegment {}.to_matchable().into()),
        (
            "UnorderedSelectStatementSegment".into(),
            UnorderedSelectStatementSegment {}.to_matchable().into(),
        ),
        (
            "FromClauseSegment".into(),
            FromClauseSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "SelectStatementSegment".into(),
            SelectStatementSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "SelectClauseModifierSegment".into(),
            SelectClauseModifierSegment {}.to_matchable().into(),
        ),
        (
            "SelectClauseElementSegment".into(),
            SelectClauseElementSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "WildcardExpressionSegment".into(),
            WildcardExpressionSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "WildcardIdentifierSegment".into(),
            WildcardIdentifierSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        ("OrderByClauseSegment".into(), OrderByClauseSegment {}.to_matchable().into()),
        (
            "TruncateStatementSegment".into(),
            TruncateStatementSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "ExpressionSegment".into(),
            ExpressionSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "ShorthandCastSegment".into(),
            ShorthandCastSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        ("DatatypeSegment".into(), DatatypeSegment { segments: Vec::new() }.to_matchable().into()),
        (
            "AliasExpressionSegment".into(),
            AliasExpressionSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "FromExpressionSegment".into(),
            FromExpressionSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "ColumnReferenceSegment".into(),
            ObjectReferenceSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "ObjectReferenceSegment".into(),
            ObjectReferenceSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "SignedSegmentGrammar".into(),
            one_of(vec![Ref::new("PositiveSegment").boxed(), Ref::new("NegativeSegment").boxed()])
                .to_matchable()
                .into(),
        ),
    ]);

    ansi_dialect.extend([
        (
            "SelectableGrammar".into(),
            one_of(vec![Ref::new("NonWithSelectableGrammar").boxed()]).to_matchable().into(),
        ),
        (
            "NonWithSelectableGrammar".into(),
            one_of(vec![
                Ref::new("SetExpressionSegment").boxed(),
                optionally_bracketed(vec![Ref::new("SelectStatementSegment").boxed()]).boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        ("NonWithNonSelectableGrammar".into(), one_of(vec![]).to_matchable().into()),
        ("NonSetSelectableGrammar".into(), one_of(vec![]).to_matchable().into()),
        (
            "ArrayAccessorSegment".into(),
            ArrayAccessorSegment { segments: vec![] }.to_matchable().into(),
        ),
        (
            "FromExpressionElementSegment".into(),
            FromExpressionElementSegment { segments: Vec::new() }.to_matchable().into(),
        ),
        (
            "IsClauseGrammar".into(),
            one_of(vec![Ref::new("NullLiteralSegment").boxed()]).to_matchable().into(),
        ),
    ]);

    ansi_dialect.extend([
        (
            "Tail_Recurse_Expression_A_Grammar".into(),
            Sequence::new(vec![
                Ref::new("Expression_A_Unary_Operator_Grammar").optional().boxed(),
                Ref::new("Expression_C_Grammar").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_A_Unary_Operator_Grammar".into(),
            one_of(vec![Ref::new("SignedSegmentGrammar").boxed()]).to_matchable().into(),
        ),
        (
            "Expression_A_Grammar".into(),
            Sequence::new(vec![
                Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                AnyNumberOf::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("BinaryOperatorGrammar").boxed(),
                            Ref::new("Tail_Recurse_Expression_A_Grammar").boxed(),
                        ])
                        .boxed(),
                        Sequence::new(vec![
                            Ref::keyword("IS").boxed(),
                            Ref::keyword("NOT").optional().boxed(),
                            Ref::new("IsClauseGrammar").boxed(),
                        ])
                        .boxed(),
                    ])
                    .boxed(),
                ])
                .boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        ("AccessorGrammar".into(), Ref::new("ArrayAccessorSegment").boxed().to_matchable().into()),
    ]);

    ansi_dialect.extend([
        (
            "BaseExpressionElementGrammar".into(),
            one_of(vec![Ref::new("LiteralGrammar").boxed(), Ref::new("ExpressionSegment").boxed()])
                .to_matchable()
                .into(),
        ),
        (
            "Expression_C_Grammar".into(),
            one_of(vec![
                Ref::new("Expression_D_Grammar").boxed(),
                Ref::new("ShorthandCastSegment").boxed(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_D_Grammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("LiteralGrammar").boxed(),
                    Ref::new("ColumnReferenceSegment").boxed(),
                ])
                .boxed(),
                Ref::new("AccessorGrammar").optional().boxed(),
            ])
            .allow_gaps(true)
            .to_matchable()
            .into(),
        ),
    ]);

    ansi_dialect.expand();
    ansi_dialect
}

fn lexer_matchers() -> Vec<Box<dyn Matcher>> {
    vec![
        // Match all forms of whitespace except newlines and carriage returns:
        // https://stackoverflow.com/questions/3469080/match-whitespace-but-not-newlines
        // This pattern allows us to also match non-breaking spaces (#2189).
        Box::new(
            RegexLexer::new(
                "whitespace",
                r"[^\S\r\n]+",
                &WhitespaceSegment::new as SegmentConstructorFn<WhitespaceSegmentNewArgs>,
                WhitespaceSegmentNewArgs {},
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "inline_comment",
                r"(--|#)[^\n]*",
                &CommentSegment::new as SegmentConstructorFn<CommentSegmentNewArgs>,
                CommentSegmentNewArgs {
                    r#type: "inline_comment",
                    trim_start: Some(vec!["--", "#"]),
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "block_comment",
                r"\/\*([^\*]|\*(?!\/))*\*\/",
                &CommentSegment::new as SegmentConstructorFn<CommentSegmentNewArgs>,
                CommentSegmentNewArgs { r#type: "block_comment", trim_start: None },
                Some(Box::new(
                    RegexLexer::new(
                        "newline",
                        r"\r\n|\n",
                        &NewlineSegment::new as SegmentConstructorFn<NewlineSegmentNewArgs>,
                        NewlineSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
                Some(Box::new(
                    RegexLexer::new(
                        "whitespace",
                        r"[^\S\r\n]+",
                        &WhitespaceSegment::new as SegmentConstructorFn<WhitespaceSegmentNewArgs>,
                        WhitespaceSegmentNewArgs {},
                        None,
                        None,
                    )
                    .unwrap(),
                )),
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "single_quote",
                r"'([^'\\]|\\.|'')*'",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "single_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "double_quote",
                r#""([^"\\]|\\.)*""#,
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "double_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "back_quote",
                r"`[^`]*`",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "back_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "dollar_quote",
                // r"\$(\w*)\$[^\1]*?\$\1\$" is the original regex, but it doesn't work in Rust.
                r"\$(\w*)\$[^\$]*?\$\1\$",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "dollar_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
                None,
                None,
            )
            .unwrap(),
        ),
        //
        // NOTE: Instead of using a created LiteralSegment and ComparisonOperatorSegment in the
        // next two, in Rust we just use a CodeSegment
        Box::new(
            RegexLexer::new(
                "numeric_literal",
                r"(?>\d+\.\d+|\d+\.(?![\.\w])|\.\d+|\d+)(\.?[eE][+-]?\d+)?((?<=\.)|(?=\b))",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "numeric_literal",
                    ..CodeSegmentNewArgs::default()
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "like_operator",
                r"!?~~?\*?",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs { code_type: "like_operator", ..CodeSegmentNewArgs::default() },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "newline",
                r"\r\n|\n",
                &NewlineSegment::new as SegmentConstructorFn<NewlineSegmentNewArgs>,
                NewlineSegmentNewArgs {},
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(StringLexer::new(
            "casting_operator",
            "::",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "casting_operator",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "equals",
            "=",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "equals",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "greater_than",
            ">",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "greater_than",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "less_than",
            "<",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "less_than",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "not",
            "!",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "not",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "dot",
            ".",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "dot",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "comma",
            ",",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "comma",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "plus",
            "+",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "plus",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "minus",
            "-",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "minus",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "divide",
            "/",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "divide",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "percent",
            "%",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "percent",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "question",
            "?",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "question",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "ampersand",
            "&",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "ampersand",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "vertical_bar",
            "|",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "vertical_bar",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "caret",
            "^",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "caret",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "star",
            "*",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "star",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_bracket",
            "(",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_bracket",
            ")",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_square_bracket",
            "[",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_square_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_square_bracket",
            "]",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_square_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "start_curly_bracket",
            "{",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "start_curly_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "end_curly_bracket",
            "}",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "end_curly_bracket",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "colon",
            ":",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "colon",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "semicolon",
            ";",
            &CodeSegment::new,
            CodeSegmentNewArgs {
                code_type: "semicolon",
                instance_types: vec![],
                trim_start: None,
                trim_chars: None,
                source_fixes: None,
            },
            None,
            None,
        )),
        //         Numeric literal matches integers, decimals, and exponential formats,
        // Pattern breakdown:
        // (?>                      Atomic grouping
        //                          (https://www.regular-expressions.info/atomic.html).
        //     \d+\.\d+             e.g. 123.456
        //     |\d+\.(?![\.\w])     e.g. 123.
        //                          (N.B. negative lookahead assertion to ensure we
        //                          don't match range operators `..` in Exasol, and
        //                          that in bigquery we don't match the "."
        //                          in "asd-12.foo").
        //     |\.\d+               e.g. .456
        //     |\d+                 e.g. 123
        // )
        // (\.?[eE][+-]?\d+)?          Optional exponential.
        // (
        //     (?<=\.)              If matched character ends with . (e.g. 123.) then
        //                          don't worry about word boundary check.
        //     |(?=\b)              Check that we are at word boundary to avoid matching
        //                          valid naked identifiers (e.g. 123column).
        // )

        // This is the "fallback" lexer for anything else which looks like SQL.
        Box::new(
            RegexLexer::new(
                "word",
                "[0-9a-zA-Z_]+",
                &CodeSegment::new,
                CodeSegmentNewArgs::default(),
                None,
                None,
            )
            .unwrap(),
        ),
    ]
}

#[derive(Default, Debug, Clone)]
pub struct FileSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl FileSegment {
    pub fn root_parse(
        &self,
        segments: &[Box<dyn Segment>],
        parse_context: &mut ParseContext,
        _f_name: Option<String>,
    ) -> Result<Box<dyn Segment>, SQLParseError> {
        // Trim the start
        let start_idx = segments.iter().position(|segment| segment.is_code()).unwrap_or(0);

        // Trim the end
        // Note: The '+ 1' in the end is to include the segment at 'end_idx' in the
        // slice.
        let end_idx =
            segments.iter().rposition(|segment| segment.is_code()).map_or(start_idx, |idx| idx + 1);

        if start_idx == end_idx {
            return Ok(Box::new(FileSegment { segments: segments.to_vec() }));
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let _closing_position = final_seg.get_position_marker().unwrap().templated_slice.end;

        let match_result = parse_context.progress_bar(|this| {
            // NOTE: Don't call .match() on the segment class itself, but go
            // straight to the match grammar inside.
            self.match_grammar()
                .unwrap()
                .match_segments(segments[start_idx..end_idx].to_vec(), this)
        })?;

        let content = if !match_result.has_match() { unimplemented!() } else { unreachable!() };
    }
}

impl Segment for FileSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Delimited::new(vec![Ref::new("StatementSegment").boxed()])
            .config(|this| {
                this.allow_gaps(true);
                this.allow_trailing(true);
                this.delimiter(
                    AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").boxed()])
                        .config(|config| config.max_times(1)),
                );
            })
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnorderedSelectStatementSegment {}

impl Segment for UnorderedSelectStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").boxed(),
            Ref::new("FromClauseSegment").optional().boxed(),
        ])
        .terminators(vec![Ref::new("OrderByClauseSegment").boxed()])
        .parse_mode(ParseMode::GreedyOnceStarted)
        .to_matchable()
        .into()
    }
}

impl Matchable for UnorderedSelectStatementSegment {}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectClauseSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for SelectClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("SELECT").boxed(),
            Ref::new("SelectClauseModifierSegment").optional().boxed(),
            Delimited::new(vec![Ref::new("SelectClauseElementSegment").boxed()])
                .config(|this| this.allow_trailing(true))
                .boxed(),
        ])
        .terminators(vec![Ref::new("SelectClauseTerminatorGrammar").boxed()])
        .parse_mode(ParseMode::GreedyOnceStarted)
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for SelectClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatementSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for StatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Ref::new("SelectableGrammar").boxed(),
            // UnorderedSelectStatementSegment {}.boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for StatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        let Some(ref grammar) = self.match_grammar() else {
            return None;
        };
        grammar.simple(parse_context, crumbs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetExpressionSegment {}

impl Segment for SetExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("NonSetSelectableGrammar").boxed()]).to_matchable().into()
    }
}

impl Matchable for SetExpressionSegment {}

#[derive(Debug, Clone, PartialEq)]
pub struct FromClauseSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for FromClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::keyword("FROM").boxed(), Ref::new("FromExpressionSegment").boxed()])
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for FromClauseSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatementSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for SelectStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        UnorderedSelectStatementSegment {}
            .match_grammar()
            .unwrap()
            .copy(
                vec![Ref::new("OrderByClauseSegment").optional().to_matchable()].into(),
                true,
                Vec::new(),
            )
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for SelectStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectClauseModifierSegment {}

impl Segment for SelectClauseModifierSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![Ref::keyword("DISTINCT").boxed(), Ref::keyword("ALL").boxed()])
            .to_matchable()
            .into()
    }
}

impl Matchable for SelectClauseModifierSegment {}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectClauseElementSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for SelectClauseElementSegment {
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            // *, blah.*, blah.blah.*, etc.
            Ref::new("WildcardExpressionSegment").boxed(),
            Sequence::new(vec![
                Ref::new("BaseExpressionElementGrammar").boxed(),
                Ref::new("AliasExpressionSegment").optional().boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }
}

impl Matchable for SelectClauseElementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct WildcardExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for WildcardExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("WildcardIdentifierSegment").boxed()]).to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for WildcardExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct WildcardIdentifierSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for WildcardIdentifierSegment {
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("StarSegment").boxed()]).allow_gaps(false).to_matchable().into()
    }
}

impl Matchable for WildcardIdentifierSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct OrderByClauseSegment {}

impl Segment for OrderByClauseSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("ORDER").boxed(),
            Ref::keyword("BY").boxed(),
            // Indent::default().boxed(),
            Delimited::new(vec![one_of(vec![Ref::new("NumericLiteralSegment").boxed()]).boxed()])
                .boxed(),
        ])
        .to_matchable()
        .into()
    }
}

impl Matchable for OrderByClauseSegment {}

#[derive(Debug, PartialEq, Clone)]
pub struct TruncateStatementSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for TruncateStatementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("TRUNCATE").boxed(),
            Ref::keyword("TABLE").optional().boxed(),
            Ref::new("TableReferenceSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for TruncateStatementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for ExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Ref::new("Expression_A_Grammar").to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShorthandCastSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for ShorthandCastSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            one_of(vec![Ref::new("Expression_D_Grammar").boxed()]).boxed(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("CastOperatorSegment").boxed(),
                    Ref::new("DatatypeSegment").boxed(),
                ])
                .boxed(),
            ])
            .config(|this| this.max_times(1))
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ShorthandCastSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatatypeSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for DatatypeSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Sequence::new(vec![
                one_of(vec![
                    Sequence::new(vec![Ref::new("DatatypeIdentifierSegment").boxed()])
                        .allow_gaps(true)
                        .boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for DatatypeSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for AliasExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::keyword("AS").optional().boxed(),
            one_of(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar").boxed()]).boxed()])
                .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for AliasExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FromExpressionSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for FromExpressionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        optionally_bracketed(vec![
            Sequence::new(vec![
                one_of(vec![Ref::new("FromExpressionElementSegment").boxed()]).boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for FromExpressionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct FromExpressionElementSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for FromExpressionElementSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![Ref::new("AliasExpressionSegment").boxed()]).to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for FromExpressionElementSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectReferenceSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for ObjectReferenceSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").boxed()]).to_matchable().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for ObjectReferenceSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

/// An array accessor e.g. [3:4].
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayAccessorSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for ArrayAccessorSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Bracketed::new(vec![
            Delimited::new(vec![
                one_of(vec![
                    Ref::new("NumericLiteralSegment").boxed(),
                    Ref::new("ExpressionSegment").boxed(),
                ])
                .boxed(),
            ])
            .config(|this| this.delimiter(Ref::new("SliceSegment")))
            .boxed(),
        ])
        .bracket_type("square")
        .to_matchable()
        .into()
    }
}

impl Matchable for ArrayAccessorSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for FunctionSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        one_of(vec![
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::new("FunctionNameSegment").boxed(),
                    Bracketed::new(vec![Ref::new("FunctionContentsGrammar").boxed()]).boxed(),
                ])
                .boxed(),
            ])
            .boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for FunctionSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionNameSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for FunctionNameSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![one_of(vec![Ref::new("FunctionNameIdentifierSegment").boxed()]).boxed()])
            .allow_gaps(false)
            .to_matchable()
            .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for FunctionNameSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct QualifiedNumericLiteralSegment {
    segments: Vec<Box<dyn Segment>>,
}

impl Segment for QualifiedNumericLiteralSegment {
    fn match_grammar(&self) -> Option<Box<dyn Matchable>> {
        Sequence::new(vec![
            Ref::new("SignedSegmentGrammar").boxed(),
            Ref::new("NumericLiteralSegment").boxed(),
        ])
        .to_matchable()
        .into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}

impl Matchable for QualifiedNumericLiteralSegment {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
        from_segments!(self, segments)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::lexer::{Lexer, StringOrTemplate};
    use crate::core::parser::segments::test_functions::{fresh_ansi_dialect, lex};

    #[test]
    fn test__dialect__ansi__file_lex() {
        // Define the test cases
        let test_cases = vec![
            ("a b", vec!["a", " ", "b", ""]),
            ("b.c", vec!["b", ".", "c", ""]),
            ("abc \n \t def  ;blah", vec!["abc", " ", "\n", " \t ", "def", "  ", ";", "blah", ""]),
        ];

        for (raw, res) in test_cases {
            // Assume FluffConfig and Lexer are defined somewhere in your codebase
            let config = FluffConfig::new(None, None, None, Some("ansi"));

            let lexer = Lexer::new(config, None);

            // Assume that the lex function returns a Result with tokens
            let tokens_result = lexer.lex(StringOrTemplate::String(raw.to_string()));

            // Check if lexing was successful, and if not, fail the test
            assert!(tokens_result.is_ok(), "Lexing failed for input: {}", raw);

            let (tokens, errors) = tokens_result.unwrap();
            assert_eq!(errors.len(), 0, "Lexing failed for input: {}", raw);

            // Check if the raw components of the tokens match the expected result
            let raw_list: Vec<String> =
                tokens.iter().map(|token| token.get_raw().unwrap()).collect();
            assert_eq!(raw_list, res, "Mismatch for input: {:?}", raw);

            // Check if the concatenated raw components of the tokens match the original raw
            // string
            let concatenated: String =
                tokens.iter().map(|token| token.get_raw().unwrap()).collect();
            assert_eq!(concatenated, raw, "Concatenation mismatch for input: {}", raw);
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_parses() {
        let cases = [
            ("SelectKeywordSegment", "select"),
            ("NakedIdentifierSegment", "online_sales"),
            ("BareFunctionSegment", "current_timestamp"),
            // ("FunctionSegment", "current_timestamp()"),
            // ("NumericLiteralSegment", "1000.0"),
            // ("ExpressionSegment", "online_sales / 1000.0"),
            //     // ("IntervalExpressionSegment", "INTERVAL 1 YEAR"),
            //     (
            //         "ExpressionSegment",
            //         "CASE WHEN id = 1 THEN 'nothing' ELSE 'test' END",
            //     ),
            //     // Nested Case Expressions
            //     (
            //        "ExpressionSegment",
            //            "CASE WHEN id = 1 THEN CASE WHEN true THEN 'something' ELSE 'nothing' END
            // ELSE 'test' END"     ),
            //     // Casting expressions
            //     ("ExpressionSegment", "CAST(ROUND(online_sales / 1000.0) AS varchar)"),
            //     // Like expressions
            //      ("ExpressionSegment", "name NOT LIKE '%y'"),
            //     // Functions with a space
            //    ("SelectClauseElementSegment", "MIN (test.id) AS min_test_id"),
            //     // Interval literals
            //     (
            //        "ExpressionSegment",
            //        "DATE_ADD(CURRENT_DATE('America/New_York'), INTERVAL 1 year)",
            //     ),
            // Array accessors
            // ("ExpressionSegment", "my_array[1]"),
            // ("ExpressionSegment", "my_array[OFFSET(1)]"),
            // ("ExpressionSegment", "my_array[5:8]"),
            // ("ExpressionSegment", "4 + my_array[OFFSET(1)]"),
            // ("ExpressionSegment", "bits[OFFSET(0)] + 7"),
            // (
            //     "SelectClauseElementSegment",
            //     (
            //         "(count_18_24 * bits[OFFSET(0)]) / audience_size AS relative_abundance"
            //     ),
            // ),
            // ("ExpressionSegment", "count_18_24 * bits[OFFSET(0)] + count_25_34"),
            // (
            //     "SelectClauseElementSegment",
            //         "(count_18_24 * bits[OFFSET(0)] + count_25_34) / audience_size AS
            // relative_abundance" ),
            // // Dense math expressions
            // ("SelectStatementSegment", "SELECT t.val/t.id FROM test WHERE id*1.0/id > 0.8"),
            // ("SelectClauseElementSegment", "t.val/t.id"),
            // // Issue with casting raise as part of PR #177
            //  ("SelectClauseElementSegment", "CAST(num AS INT64)"),
            // // Casting as datatype with arguments
            // ("SelectClauseElementSegment", "CAST(num AS numeric(8,4))"),
            // // Wildcard field selection
            // ("SelectClauseElementSegment", "a.*"),
            // ("SelectClauseElementSegment", "a.b.*"),
            // ("SelectClauseElementSegment", "a.b.c.*"),
            // // Default Element Syntax
            // ("SelectClauseElementSegment", "a..c.*"),
            // // Negative Elements
            ("SelectClauseElementSegment", "-some_variable"),
            ("SelectClauseElementSegment", "- some_variable"),
            // // Complex Functions
            // (
            //     "ExpressionSegment",
            //     "concat(left(uaid, 2), '|', right(concat('0000000', SPLIT_PART(uaid, '|', 4)),
            // 10), '|', '00000000')" ),
            // // Notnull and Isnull
            // ("ExpressionSegment", "c is null"),
            // ("ExpressionSegment", "c is not null"),
            // ("SelectClauseElementSegment", "c is null as c_isnull"),
            // ("SelectClauseElementSegment", "c is not null as c_notnull"),
            // // Shorthand casting
            ("ExpressionSegment", "NULL::INT"),
            ("SelectClauseElementSegment", "NULL::INT AS user_id"),
            ("TruncateStatementSegment", "TRUNCATE TABLE test"),
            ("TruncateStatementSegment", "TRUNCATE test"),
        ];

        for (segment_ref, sql_string) in cases {
            let dialect = fresh_ansi_dialect();
            let mut ctx = ParseContext::new(dialect.clone());

            let segment = dialect.r#ref(segment_ref);
            let mut segments = lex(sql_string);

            if segments.last().unwrap().get_type() == "EndOfFile" {
                segments.pop();
            }

            let mut match_result = segment.match_segments(segments, &mut ctx).unwrap();

            assert_eq!(match_result.len(), 1, "failed {segment_ref}");

            let parsed = match_result.matched_segments.pop().unwrap();
            assert_eq!(sql_string, parsed.get_raw().unwrap());
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_not_match() {
        let cases = [("ObjectReferenceSegment", "\n     ")];

        let dialect = fresh_ansi_dialect();
        for (segment_ref, sql) in cases {
            let config = FluffConfig::new(None, None, None, None);
            let segments = lex(sql);

            let mut parse_cx = ParseContext::from_config(config);
            let segment = dialect.r#ref(segment_ref);

            let match_result = segment.match_segments(segments, &mut parse_cx).unwrap();
            assert!(!match_result.has_match());
        }
    }

    #[test]
    fn test__dialect__ansi_specific_segment_not_parse() {
        let tests = vec![
            ("SELECT 1 + (2 ", vec![(1, 12)]),
            // (
            //     "SELECT * FROM a ORDER BY 1 UNION SELECT * FROM b",
            //     vec![(1, 28)],
            // ),
            // (
            //     "SELECT * FROM a LIMIT 1 UNION SELECT * FROM b",
            //     vec![(1, 25)],
            // ),
            // (
            //     "SELECT * FROM a ORDER BY 1 LIMIT 1 UNION SELECT * FROM b",
            //     vec![(1, 36)],
            // ),
        ];

        for (raw, err_locations) in tests {
            let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);
            let parsed = lnt.parse_string(raw.to_string(), None, None, None, None).unwrap();
            assert!(!parsed.violations.is_empty());

            let locs: Vec<(usize, usize)> =
                parsed.violations.iter().map(|v| (v.line_no, v.line_pos)).collect();
            assert_eq!(locs, err_locations);
        }
    }

    #[test]
    #[ignore = "WIP"]
    fn test__dialect__ansi_is_whitespace() {
        let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);
        let file_content =
            std::fs::read_to_string("test/fixtures/dialects/ansi/select_in_multiline_comment.sql")
                .expect("Unable to read file");

        let parsed = lnt.parse_string(file_content, None, None, None, None).unwrap();

        for _raw_seg in parsed.tree.unwrap().get_raw_segments().unwrap() {
            unimplemented!()
            // if raw_seg.is_type("whitespace", "newline") {
            //     assert!(raw_seg.is_whitespace());
            // }
        }
    }

    #[test]
    #[ignore = "WIP"]
    fn test__dialect__ansi_parse_indented_joins() {
        let cases = [("select field_1 from my_table as alias_1",)];
        let lnt = Linter::new(FluffConfig::new(None, None, None, None), None, None);

        for (sql_string,) in cases {
            let parsed = lnt.parse_string(sql_string.to_string(), None, None, None, None).unwrap();
        }
    }
}
