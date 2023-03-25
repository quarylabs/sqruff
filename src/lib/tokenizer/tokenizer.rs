use std::{iter::Peekable, str::Chars};

use crate::dialect::Dialect;

pub enum Token {
    Keyword(String),
    QuotedString(String),
    Number(String),
    Comma,
    WhiteSpace(WhiteSpace),
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

pub enum WhiteSpace {
    Space,
    Newline,
    Tab,
    SingleLineComment,
    MultilineComment,
}

pub struct TokenizerError(String);

pub struct Tokenizer<'a> {
    dialect: &'a dyn Dialect,
    pub query: String,
    pub line: u64,
    pub col: u64,
}

impl<'a> Tokenizer<'a> {
    pub fn new(dialect:&'a dyn Dialect, query: &str) -> Self {
        Tokenizer {
            dialect,
            query: query.to_string(),
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, TokenizerError> {
        let mut tokens = vec![];
        let mut peekable = self.query.chars().peekable();
        while let Some(token) = self.next_token(&mut peekable) {
            match &token {
                Token::WhiteSpace(WhiteSpace::Newline) => {
                    self.col = 1;
                    self.line += 1;
                },
                Token::WhiteSpace(WhiteSpace::Tab) => self.col += 4,
                Token::Keyword(w) => self.col += w.len() as u64,
                Token::Number(n) => self.col += n.len() as u64,
                _ => self.col += 1,
            }
            tokens.push(token);
        }
        Ok(tokens)
    }

    pub fn next_token(&self, chars: &mut Peekable<Chars<'_>>) -> Option<Token> {
        match chars.peek() {
            Some(&ch) => match ch {
                ' ' => self.consume_and_return( chars,Token::WhiteSpace(WhiteSpace::Space)),
                _ => None,
            },
            None => None,
        }
    }

    fn consume_and_return(&self, chars:&mut Peekable<Chars<'_>>, t: Token) -> Option<Token> {
        chars.next();
        Some(t)
    }
}
