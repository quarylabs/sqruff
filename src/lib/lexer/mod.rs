static RESERVED_WORDS:&'static [&str; 6] = &["select","create","update","delete", "from", "where"];

#[derive(Debug)]
pub enum TokenKind {
    ReservedWord(String),
    Identifier(String),
    Constant(String),
    Operator(String),
    Delimiter(String),
    EOF,
}

#[derive(Debug)]
pub struct Token{
    pub kind: TokenKind,
}

impl Token {
    pub fn tokenize(token: &str) -> Option<Token> {
        let kind: TokenKind;
        // Is it a reserved word ?
        if Token::is_reserved(token) == true { 
            kind = TokenKind::ReservedWord(token.to_lowercase());
        } else if Token::is_delimiter(token) {
            kind = TokenKind::Delimiter(String::from(token));
        } else if Token::is_operator(token) {
            kind = TokenKind::Operator(String::from(token));
        } else if Token::is_constant(token) {
            kind = TokenKind::Constant(String::from(token));
        } else {
            kind = TokenKind::Identifier(String::from(token));
        }
        Some(Token { kind })
    }

    pub fn is_reserved(token: &str) -> bool {
        RESERVED_WORDS.contains(&token.to_lowercase().as_str())
    }

    pub fn is_delimiter(token: &str) -> bool {
        match token {
            "," | " " | "(" | ")" | "[" | "]" => true,
            _ => false,
        }
    }

    pub fn is_operator(token: &str) -> bool {
        match token {
            "+" | "-" | ">" | "<" | ">=" | "<=" => true,
            _ => false,
        }
    }

    pub fn is_constant(token: &str) -> bool {
        match token.parse::<f64>() {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn is_identifier(token: &str) -> bool {
        match token {
            "*" => true,
            _ => false,
        }
    }

}

pub struct Lexer {
    pub query: String,
    pub current_position: u128, // current position of the lexer
}

impl Iterator for Lexer {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        // Read the next token
        let mut token_str = String::new();
        if self.current_position < self.query.len() as u128 - 1 {
            let mut idx = 0;
            loop {
                let c = self.query.chars().collect::<Vec<char>>()[self.current_position as usize];
                if c.is_ascii() {
                    match c {
                        ' ' | ';' | ',' => {
                            if idx == 0 {
                                token_str.push(c);
                                self.current_position += 1;
                                break;
                            } else {
                                break;
                            }
                        },
                        _ => {
                            self.current_position += 1;
                            token_str.push_str(c.to_string().as_str());
                        },
                    }
                } else {
                    panic!("Non ASCII character encountered; {}", c);
                }
                idx += 1;
            }
        } else { return None };
        let token = Token::tokenize(&token_str);
        return token;
    }
}

impl Lexer {
    pub fn new(query: &str) -> Lexer {
        Lexer {
            query: String::from(query),
            current_position: 0,
        }
    }

    pub fn lex(&mut self) -> Result<String, String>{
        loop {
            if let Some(token) = self.next() {
                println!("{:?}", token.kind);
            }
            else { break; }
        }
        Ok(String::from("Finished!"))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_lexer() {
        let lexer: Lexer = Lexer::new("SELECT * FROM table;");
         assert_eq!(&lexer.query, "SELECT * FROM table;");
    }

    #[test]
    fn lex() {
        let mut lexer: Lexer = Lexer::new("SELECT * FROM table;");
        assert_eq!(lexer.lex(), Ok(String::from("Finished!")));
    }
}
