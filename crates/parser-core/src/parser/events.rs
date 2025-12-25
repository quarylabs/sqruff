use smol_str::SmolStr;

use crate::dialects::syntax::SyntaxKind;
use crate::parser::core::{EventSink, Token};

pub trait ParseEventHandler {
    fn enter_node(&mut self, kind: SyntaxKind);
    fn exit_node(&mut self, kind: SyntaxKind);
    fn token(&mut self, kind: SyntaxKind, raw: &str);
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseEvent {
    EnterNode { kind: SyntaxKind },
    ExitNode { kind: SyntaxKind },
    Token { kind: SyntaxKind, raw: SmolStr },
}

#[derive(Debug, Default)]
pub struct EventCollector {
    events: Vec<ParseEvent>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn events(&self) -> &[ParseEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<ParseEvent> {
        self.events
    }
}

impl ParseEventHandler for EventCollector {
    fn enter_node(&mut self, kind: SyntaxKind) {
        self.events.push(ParseEvent::EnterNode { kind });
    }

    fn exit_node(&mut self, kind: SyntaxKind) {
        self.events.push(ParseEvent::ExitNode { kind });
    }

    fn token(&mut self, kind: SyntaxKind, raw: &str) {
        self.events.push(ParseEvent::Token {
            kind,
            raw: raw.into(),
        });
    }
}

impl EventSink for EventCollector {
    fn enter_node(&mut self, kind: SyntaxKind) {
        self.events.push(ParseEvent::EnterNode { kind });
    }

    fn exit_node(&mut self, kind: SyntaxKind) {
        self.events.push(ParseEvent::ExitNode { kind });
    }

    fn token(&mut self, token: &Token) {
        self.events.push(ParseEvent::Token {
            kind: token.kind,
            raw: token.raw.clone(),
        });
    }
}

pub struct ParseEventHandlerSink<'a, H: ParseEventHandler> {
    handler: &'a mut H,
}

impl<'a, H: ParseEventHandler> ParseEventHandlerSink<'a, H> {
    pub fn new(handler: &'a mut H) -> Self {
        Self { handler }
    }
}

impl<'a, H: ParseEventHandler> EventSink for ParseEventHandlerSink<'a, H> {
    fn enter_node(&mut self, kind: SyntaxKind) {
        self.handler.enter_node(kind);
    }

    fn exit_node(&mut self, kind: SyntaxKind) {
        self.handler.exit_node(kind);
    }

    fn token(&mut self, token: &Token) {
        self.handler.token(token.kind, token.raw.as_ref());
    }
}
