use lib_sqlfluff::lexer::Lexer;

fn main() {
    let mut lexer = Lexer::new("SELECT * FROM table;");
    lexer.lex();
}
