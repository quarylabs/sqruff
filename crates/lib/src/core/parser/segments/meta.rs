use std::collections::HashSet;

use uuid::Uuid;

use super::base::CloneSegment;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
use crate::helpers::Boxed;

/// A segment which is empty but indicates where an indent should be.
///
///     This segment is always empty, i.e. its raw format is '', but it
/// indicates     the position of a theoretical indent which will be used in
/// linting     and reconstruction. Even if there is an *actual indent* that
/// occurs     in the same place this intentionally *won't* capture it, they
/// will just     be compared later.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct Indent {
    pub indent_val: usize,
    pub is_implicit: bool,
    position_marker: Option<PositionMarker>,
    uuid: Uuid,
}

impl Default for Indent {
    fn default() -> Self {
        Self { indent_val: 1, is_implicit: false, position_marker: None, uuid: Uuid::new_v4() }
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
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        self.clone_box()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone_box()]
    }

    fn get_type(&self) -> &'static str {
        "indent"
    }

    fn is_meta(&self) -> bool {
        true
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
        self.position_marker.clone().into()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_raw(&self) -> Option<String> {
        String::new().into()
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
#[derive(Hash, Debug, Clone, PartialEq)]
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

#[derive(Hash, Clone, Debug, PartialEq)]
pub struct EndOfFile {
    uuid: Uuid,
    position_maker: PositionMarker,
}

impl EndOfFile {
    pub fn new(position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(EndOfFile { position_maker, uuid: Uuid::new_v4() })
    }
}

impl Segment for EndOfFile {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { uuid: self.uuid, position_maker: self.position_maker.clone() }.boxed()
    }

    fn get_raw(&self) -> Option<String> {
        Some(String::new())
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn get_type(&self) -> &'static str {
        "end_of_file"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}
