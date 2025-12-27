use crate::dialects::SyntaxKind;
use crate::parser::token::Token;

pub trait EventSink {
    fn enter_node(&mut self, kind: SyntaxKind, estimated_children: usize);
    fn exit_node(&mut self, kind: SyntaxKind);
    fn token(&mut self, token: &Token);
}
