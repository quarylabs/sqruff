use itertools::Itertools;

use crate::add_to_dialect;
use crate::core::dialects::base::{BracketPair, Dialect};
use crate::core::parser::context::ParseContext;
use crate::core::parser::grammar::base::Ref;
use crate::core::parser::lexer::{Matcher, RegexLexer, StringLexer};
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::keyword::KeywordSegment;

use super::ansi_keywords::{ANSI_RESERVED_KEYWORDS, ANSI_UNRESERVED_KEYWORDS};
use crate::core::parser::parsers::RegexParser;
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, NewlineSegment,
    NewlineSegmentNewArgs, Segment, SegmentConstructorFn, WhitespaceSegment,
    WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::types::DialectElementType;

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

    ansi_dialect
        .sets_mut("date_part_function_name")
        .extend(["DATEADD"]);

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
            BracketPair {
                bracket_type: "round".to_string(),
                start_ref: "StartBracketSegment".to_string(),
                end_ref: "EndBracketSegment".to_string(),
                persists: true,
            },
            BracketPair {
                bracket_type: "square".to_string(),
                start_ref: "StartSquareBracketSegment".to_string(),
                end_ref: "EndSquareBracketSegment".to_string(),
                persists: false,
            },
            BracketPair {
                bracket_type: "curly".to_string(),
                start_ref: "StartCurlyBracketSegment".to_string(),
                end_ref: "EndCurlyBracketSegment".to_string(),
                persists: false,
            },
        ],
    );

    // Set the value table functions. These are functions that, if they appear as
    // an item in "FROM", are treated as returning a COLUMN, not a TABLE. Apparently,
    // among dialects supported by SQLFluff, only BigQuery has this concept, but this
    // set is defined in the ANSI dialect because:
    // - It impacts core linter rules (see AL04 and several other rules that subclass
    //   from it) and how they interpret the contents of table_expressions
    // - At least one other database (DB2) has the same value table function,
    //   UNNEST(), as BigQuery. DB2 is not currently supported by SQLFluff.
    ansi_dialect.sets_mut("value_table_functions");

    add_to_dialect!(ansi_dialect,
        "DelimiterGrammar" => {
            DialectElementType::Matchable(Box::new(Ref::new("SemicolonSegment".into(), None, Vec::new(), false, false, false)))
        },
        "NakedIdentifierSegment" => {
            DialectElementType::SegmentGenerator(SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");

                // Join the keywords with the pipe symbol
                let pattern = reserved_keywords.iter().join("|");

                // Format the string to include the regex anchors
                let anti_template = format!("^({})$", pattern);

                Box::new(RegexParser::new(
                    "[A-Z0-9_]*[A-Z][A-Z0-9_]*",
                    |segment| Box::new(KeywordSegment::new(segment.get_raw().unwrap())),
                    None,
                    false,
                    anti_template.into(),
                    None,
                ))
            }))
        },
        "ParameterNameSegment" => {
            DialectElementType::SegmentGenerator(SegmentGenerator::new(|_dialect| {
                // Define the regex pattern for the parameter name segment
                let pattern = r#"\"?[A-Z][A-Z0-9_]*\"?"#;

                Box::new(RegexParser::new(
                    pattern,
                    |_| todo!(),
                    None,
                    false,
                    None,
                    None,
                ))
            }))
        }
    );

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
                CommentSegmentNewArgs {
                    r#type: "block_comment",
                    trim_start: None,
                },
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
        // NOTE: Instead of using a created LiteralSegment and ComparisonOperatorSegment in the next two,
        // in Rust we just use a CodeSegment
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
                CodeSegmentNewArgs {
                    code_type: "like_operator",
                    ..CodeSegmentNewArgs::default()
                },
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
        _parse_context: ParseContext,
        _f_name: Option<String>,
    ) -> Box<dyn Segment> {
        // Trim the start
        let start_idx = segments
            .iter()
            .position(|segment| segment.is_code())
            .unwrap_or(0);

        // Trim the end
        // Note: The '+ 1' in the end is to include the segment at 'end_idx' in the slice.
        let end_idx = segments
            .iter()
            .rposition(|segment| segment.is_code())
            .map_or(start_idx, |idx| idx + 1);

        if start_idx == end_idx {
            return Box::new(FileSegment {
                segments: segments.to_vec(),
            });
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let _closing_position = final_seg.get_position_marker().unwrap().templated_slice.end;

        unimplemented!()
    }
}

impl Segment for FileSegment {
    fn get_raw(&self) -> Option<String> {
        self.segments
            .iter()
            .filter_map(|segment| segment.get_raw())
            .join("")
            .into()
    }

    fn get_type(&self) -> &'static str {
        todo!()
    }

    fn is_code(&self) -> bool {
        todo!()
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        todo!()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<uuid::Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<crate::core::parser::segments::fix::SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::parser::lexer::{Lexer, StringOrTemplate};

    #[test]
    fn test_dialect_ansi_file_lex() {
        // Define the test cases
        let test_cases = vec![
            ("a b", vec!["a", " ", "b", ""]),
            ("b.c", vec!["b", ".", "c", ""]),
            (
                "abc \n \t def  ;blah",
                vec!["abc", " ", "\n", " \t ", "def", "  ", ";", "blah", ""],
            ),
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
            let raw_list: Vec<String> = tokens
                .iter()
                .map(|token| token.get_raw().unwrap())
                .collect();
            assert_eq!(raw_list, res, "Mismatch for input: {:?}", raw);

            // Check if the concatenated raw components of the tokens match the original raw string
            let concatenated: String = tokens
                .iter()
                .map(|token| token.get_raw().unwrap())
                .collect();
            assert_eq!(
                concatenated, raw,
                "Concatenation mismatch for input: {}",
                raw
            );
        }
    }
}
