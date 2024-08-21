use std::collections::HashMap;

use sqruff_lib::core::dialects::init::DialectKind;
use sqruff_lib::core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib::dialects::SyntaxKind;
use sqruff_lib::helpers::Config;

fn traverse(
    segment: &mut ErasedSegment,
    f: &mut impl Fn(&mut ErasedSegment, &HashMap<String, ErasedSegment>),
    sources: &HashMap<String, ErasedSegment>,
) {
    let segments = segment.get_mut().segments();

    for segment in segments {
        f(segment, sources);

        for segment in segment.get_mut().segments() {
            traverse(segment, f, sources);
        }
    }
}

pub(crate) fn expand(
    mut segment: ErasedSegment,
    sources: &HashMap<String, ErasedSegment>,
) -> ErasedSegment {
    traverse(&mut segment, &mut expand_inner, sources);
    segment
}

fn expand_inner(segment: &mut ErasedSegment, sources: &HashMap<String, ErasedSegment>) {
    if segment.get_type() == SyntaxKind::TableReference {
        if let Some(source) = sources.get(&segment.raw().to_string()) {
            let mut new_node = source.deep_clone2();
            new_node.set_dummy_span();

            let new_node = SegmentBuilder::node(
                0,
                SyntaxKind::Bracketed,
                DialectKind::Ansi,
                vec![
                    SegmentBuilder::token(0, "(", SyntaxKind::StartBracket).finish(),
                    new_node,
                    SegmentBuilder::token(0, ")", SyntaxKind::StartBracket).finish(),
                    SegmentBuilder::token(0, " ", SyntaxKind::Whitespace).finish(),
                    SegmentBuilder::node(
                        0,
                        SyntaxKind::AliasExpression,
                        DialectKind::Ansi,
                        vec![
                            SegmentBuilder::keyword(0, "AS"),
                            SegmentBuilder::token(0, " ", SyntaxKind::Whitespace).finish(),
                            SegmentBuilder::token(0, &segment.raw(), SyntaxKind::NakedIdentifier)
                                .finish(),
                        ],
                    )
                    .finish(),
                ],
            )
            .finish();

            let mut new_node = expand(new_node, sources);
            new_node.set_dummy_span();
            *segment = new_node;
        }
    }
}

trait DummySpan {
    fn set_dummy_span(&mut self);
}

impl DummySpan for ErasedSegment {
    fn set_dummy_span(&mut self) {
        for segment in self.get_mut().segments() {
            segment.set_dummy_span();
        }

        self.get_mut().config(|this| this.set_position_marker(Some(Default::default())));
    }
}
