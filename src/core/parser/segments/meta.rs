use std::collections::HashSet;

use uuid::Uuid;

use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;

/// A segment which is empty but indicates where an indent should be.
///
///     This segment is always empty, i.e. its raw format is '', but it
/// indicates     the position of a theoretical indent which will be used in
/// linting     and reconstruction. Even if there is an *actual indent* that
/// occurs     in the same place this intentionally *won't* capture it, they
/// will just     be compared later.
#[derive(Debug, Clone, PartialEq)]
pub struct Indent {
    pub indent_val: usize,
}

impl Default for Indent {
    fn default() -> Self {
        Self { indent_val: 1 }
    }
}

pub struct IndentNewArgs {}

impl Matchable for Indent {
    fn is_optional(&self) -> bool {
        todo!()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        todo!()
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        todo!()
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

impl Segment for Indent {
    fn get_type(&self) -> &'static str {
        "indent"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn get_raw(&self) -> Option<String> {
        todo!()
    }
}

impl Indent {
    pub fn new(_position_maker: PositionMarker) -> Self {
        Indent::default()
    }
}

/// A segment which is empty but indicates where an dedent should be.
///
/// This segment is always empty, i.e. its raw format is '', but it
/// indicates the position of a theoretical dedent which will be used in
/// linting and reconstruction. Even if there is an *actual dedent* that
/// occurs in the same place this intentionally *won't* capture it, they
/// will just be compared later.
#[derive(Debug, Clone)]
pub struct Dedent {}

pub struct DedentNewArgs {}

impl Segment for Dedent {
    fn get_type(&self) -> &'static str {
        "dedent"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }

    fn get_raw(&self) -> Option<String> {
        todo!()
    }
}

impl Dedent {
    pub fn new(_position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(Dedent {})
    }
}

#[derive(Clone, Debug)]
pub struct EndOfFile {
    position_maker: PositionMarker,
}

impl EndOfFile {
    pub fn new(position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(EndOfFile { position_maker })
    }
}

impl Segment for EndOfFile {
    fn get_raw(&self) -> Option<String> {
        Some(String::new())
    }

    fn get_type(&self) -> &'static str {
        "EndOfFile"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        todo!()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}
