use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_dialects::tsql;

fn main() {
    let sql = r#"CREATE PROCEDURE findjobs @nm sysname = NULL
AS
IF @nm IS NULL
    BEGIN
        PRINT 'You must give a user name'
        RETURN
    END
ELSE
    BEGIN
        SELECT o.name, o.id, o.uid
        FROM sysobjects o INNER JOIN master..syslogins l
            ON o.uid = l.sid
        WHERE l.name = @nm
    END;"#;

    let dialect = tsql::dialect();
    let lexer = Lexer::new(&dialect, None);
    let tokens = lexer.lex(&sql.into()).unwrap();
    
    let mut parser = Parser::new(&dialect, tokens.into());
    let segments = parser.parse();
    
    println!("Parsed segments:\n{:#?}", segments.unwrap());
}