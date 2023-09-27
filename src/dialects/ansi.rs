use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::{Matcher, RegexLexer, StringLexer};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, NewLineSegmentNewArgs,
    NewlineSegment, SegmentConstructorFn, WhitespaceSegment, WhitespaceSegmentNewArgs,
};

#[derive(Debug)]
pub struct AnsiDialect;

impl Dialect for AnsiDialect {
    fn get_lexer_matchers(&self) -> Vec<Box<dyn Matcher>> {
        lexer_matchers()
    }
}

// TODO This can probably be turned into a const fn
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
                        &NewlineSegment::new as SegmentConstructorFn<NewLineSegmentNewArgs>,
                        NewLineSegmentNewArgs {},
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
                },
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(
            RegexLexer::new(
                "dollar_quote",
                r"\$(\w*)\$[^\1]*?\$\1\$",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "dollar_quote",
                },
                None,
                None,
            )
            .unwrap(),
        ),
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
        //
        // NOTE: Instead of using a created LiteralSegment and ComparisonOperatorSegment in the next two,
        // in Rust we just use a CodeSegment
        Box::new(
            RegexLexer::new(
                "numeric_literal",
                r"\$(\w*)\$[^\1]*?\$\1\$",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs {
                    code_type: "numeric_literal",
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
                &NewlineSegment::new as SegmentConstructorFn<NewLineSegmentNewArgs>,
                NewLineSegmentNewArgs {},
                None,
                None,
            )
            .unwrap(),
        ),
        Box::new(StringLexer::new(
            "casting_operator",
            "::",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "equals",
            "=",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "greater_than",
            ">",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "less_than",
            "<",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "not",
            "!",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "dot",
            ".",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "comma",
            ",",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "comma" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "plus",
            "+",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "minus",
            "-",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "divide",
            "/",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "percent",
            "%",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "question",
            "?",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "ampersand",
            "&",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "vertical_bar",
            "|",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "caret",
            "^",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "star",
            "*",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "bracket_open",
            "(",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "bracket_close",
            ")",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "sq_bracket_open",
            "[",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "sq_bracket_close",
            "]",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "crly_bracket_open",
            "{",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "crly_bracket_close",
            "}",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "colon",
            ":",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        Box::new(StringLexer::new(
            "semicolon",
            ";",
            &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
            CodeSegmentNewArgs { code_type: "" },
            None,
            None,
        )),
        // This is the "fallback" lexer for anything else which looks like SQL.
        Box::new(
            RegexLexer::new(
                "code",
                r"[0-9a-zA-Z_]+",
                &CodeSegment::new as SegmentConstructorFn<CodeSegmentNewArgs>,
                CodeSegmentNewArgs { code_type: "code" },
                None,
                None,
            )
            .unwrap(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    // TODO Implement Test:
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
            let raw_list: Vec<&str> = tokens
                .iter()
                .map(|token| token.get_raw().unwrap())
                .collect();
            assert_eq!(raw_list, res, "Mismatch for input: {}", raw);

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
