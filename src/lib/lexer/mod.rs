mod tokens;
use tokens::{ Token, TokenKind};

pub struct Lexer {
    pub query: String,
    pub current_position: u128, // current position of the lexer
}

impl Iterator for Lexer {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {

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
        while let Some(token) = self.next() {
            // println!("{:?}", token);
        }
        Ok(String::from("Finished!"))
    }

    pub fn read_next_token(&mut self) {
       let reading = true;
       while reading {
           let mut index = 0;
           let read_token = false;
           let mut token_chars: Vec<char> = vec![];
           while !read_token {
               let ch = self.query.chars().collect::<Vec<char>>()[self.current_position as usize];
           }
       }
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
