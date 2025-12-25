use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::parser::markers::PositionMarker;
use crate::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use crate::templaters::TemplatedFile;
use sqruff_parser_core::parser::core::{EventSink, Token};

pub struct SegmentTreeBuilder<'a> {
    dialect: DialectKind,
    tables: &'a Tables,
    templated_file: TemplatedFile,
    stack: Vec<NodeFrame>,
    root: Option<ErasedSegment>,
}

struct NodeFrame {
    kind: SyntaxKind,
    children: Vec<ErasedSegment>,
}

impl<'a> SegmentTreeBuilder<'a> {
    pub fn new(dialect: DialectKind, tables: &'a Tables, templated_file: TemplatedFile) -> Self {
        Self {
            dialect,
            tables,
            templated_file,
            stack: Vec::new(),
            root: None,
        }
    }

    pub fn finish(mut self) -> Option<ErasedSegment> {
        debug_assert!(self.stack.is_empty(), "unfinished node stack");
        self.root.take()
    }

    fn push_segment(&mut self, segment: ErasedSegment) {
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(segment);
        } else if self.root.is_none() {
            self.root = Some(segment);
        } else {
            panic!("Multiple root segments emitted");
        }
    }
}

impl<'a> EventSink for SegmentTreeBuilder<'a> {
    fn enter_node(&mut self, kind: SyntaxKind) {
        self.stack.push(NodeFrame {
            kind,
            children: Vec::new(),
        });
    }

    fn exit_node(&mut self, kind: SyntaxKind) {
        let frame = self.stack.pop().expect("exit_node without enter_node");
        assert_eq!(frame.kind, kind, "exit_node kind mismatch");

        let node = SegmentBuilder::node(self.tables.next_id(), kind, self.dialect, frame.children)
            .position_from_segments()
            .finish();

        self.push_segment(node);
    }

    fn token(&mut self, token: &Token) {
        let position = PositionMarker::new(
            token.span.source_range(),
            token.span.templated_range(),
            self.templated_file.clone(),
            None,
            None,
        );

        let segment = SegmentBuilder::token(self.tables.next_id(), token.raw.as_ref(), token.kind)
            .with_position(position)
            .finish();

        self.push_segment(segment);
    }
}
