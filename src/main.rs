use lib_sqlfluff::lexer::Lexer;


fn main() {
    let mut lexer = Lexer::new("SELECT username,email FROM table;");
    lexer.lex();
}
