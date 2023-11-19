use crate::{core::config::FluffConfig, dialects::ansi::FileSegment};

use super::{context::ParseContext, segments::base::Segment};

/// Instantiates parsed queries from a sequence of lexed raw segments.
pub struct Parser {
    config: FluffConfig,
    root_segment: FileSegment,
}

impl Parser {
    pub fn new(_config: Option<FluffConfig>, _dialect: Option<String>) -> Self {
        // TODO:
        // let config = FluffConfig::from_kwargs(config, dialect, None);
        let config = FluffConfig::new(None, None, None, None);

        Self {
            config,
            root_segment: FileSegment {},
        }
    }

    pub fn parse(
        &mut self,
        segments: &[Box<dyn Segment>],
        f_name: String,
        parse_statistics: bool,
    ) -> Option<Box<dyn Segment>> {
        if segments.is_empty() {
            // This should normally never happen because there will usually
            // be an end_of_file segment. It would probably only happen in
            // api use cases.
            return None;
        }

        // NOTE: This is the only time we use the parse context not in the
        // context of a context manager. That's because it's the initial
        // instantiation.
        let ctx = ParseContext::from_config(self.config.clone());
        // Kick off parsing with the root segment. The BaseFileSegment has
        // a unique entry point to facilitate exaclty this. All other segments
        // will use the standard .match()/.parse() route.
        let root = self.root_segment.root_parse(segments, ctx, f_name.into());

        if parse_statistics {
            unimplemented!();
        }

        root.into()
    }
}
