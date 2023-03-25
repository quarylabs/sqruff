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

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Keyword(ref w) => write!(f, "{}", w),
            _ => write!("NUN"),
        }
    }
}
