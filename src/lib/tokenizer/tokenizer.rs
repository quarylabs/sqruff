use crate::dialect::Dialect;

pub enum Token {
    Keyword(String),
    QuotedString(String),
    Number(String),
    Comma,
    WhiteSpace,
    Eq,
    Neq,
    Lt,
    Gt,
    GtEl,
    LtEq,
    Plus,
    Minus,
    Div,
    Mult,
    Mod,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Ampersand,
    SemiColon,
    Colon,
    DoubleColon,
    Period,
}

pub struct TokenizerError(String);

pub struct Tokenizer {
    dialect: &'a dyn Dialect,
    pub query: String,
    pub line: u64,
    pub col: u64,
}

impl<'a> Tokenizer<'a> {
    pub fn new(dialect:&'a Dialect, query: &str) -> Self {
        Tokenizer {
            dialect,
            query: query.to_string(),
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize() -> Result<Vec<Token>, TokenizerError> {
        Ok(vec![])
    }

    pub fn next_token() -> Option<Token> {
        None
    }
}
