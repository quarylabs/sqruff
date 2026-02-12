pub mod compiled;
pub mod context;
pub mod grammar;
pub mod lexer;
pub mod lookahead;
pub mod markers;
pub mod match_algorithms;
pub mod match_result;
pub mod matchable;
pub mod node_matcher;
pub mod parsers;
pub mod segments;
pub mod types;

use crate::dialects::Dialect;
use crate::errors::SQLParseError;
use segments::{ErasedSegment, Tables};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndentationConfig {
    flags: u16,
}

impl IndentationConfig {
    const INDENTED_JOINS_KEY: &'static str = "indented_joins";
    const INDENTED_USING_ON_KEY: &'static str = "indented_using_on";
    const INDENTED_ON_CONTENTS_KEY: &'static str = "indented_on_contents";
    const INDENTED_THEN_KEY: &'static str = "indented_then";
    const INDENTED_THEN_CONTENTS_KEY: &'static str = "indented_then_contents";
    const INDENTED_JOINS_ON_KEY: &'static str = "indented_joins_on";
    const INDENTED_CTES_KEY: &'static str = "indented_ctes";

    pub const INDENTED_JOINS: Self = Self { flags: 1 << 0 };
    pub const INDENTED_USING_ON: Self = Self { flags: 1 << 1 };
    pub const INDENTED_ON_CONTENTS: Self = Self { flags: 1 << 2 };
    pub const INDENTED_THEN: Self = Self { flags: 1 << 3 };
    pub const INDENTED_THEN_CONTENTS: Self = Self { flags: 1 << 4 };
    pub const INDENTED_JOINS_ON: Self = Self { flags: 1 << 5 };
    pub const INDENTED_CTES: Self = Self { flags: 1 << 6 };

    pub fn from_bool_lookup(mut get: impl FnMut(&str) -> bool) -> Self {
        let mut config = Self::default();

        config.insert_if(Self::INDENTED_JOINS, get(Self::INDENTED_JOINS_KEY));
        config.insert_if(Self::INDENTED_USING_ON, get(Self::INDENTED_USING_ON_KEY));
        config.insert_if(
            Self::INDENTED_ON_CONTENTS,
            get(Self::INDENTED_ON_CONTENTS_KEY),
        );
        config.insert_if(Self::INDENTED_THEN, get(Self::INDENTED_THEN_KEY));
        config.insert_if(
            Self::INDENTED_THEN_CONTENTS,
            get(Self::INDENTED_THEN_CONTENTS_KEY),
        );
        config.insert_if(Self::INDENTED_JOINS_ON, get(Self::INDENTED_JOINS_ON_KEY));
        config.insert_if(Self::INDENTED_CTES, get(Self::INDENTED_CTES_KEY));

        config
    }

    pub fn contains(self, required: Self) -> bool {
        (self.flags & required.flags) == required.flags
    }

    pub fn insert(&mut self, flag: Self) {
        self.flags |= flag.flags;
    }

    pub fn insert_if(&mut self, flag: Self, enabled: bool) {
        if enabled {
            self.insert(flag);
        }
    }
}

#[derive(Clone)]
pub struct Parser<'a> {
    dialect: &'a Dialect,
    pub(crate) indentation_config: IndentationConfig,
}

impl<'a> From<&'a Dialect> for Parser<'a> {
    fn from(value: &'a Dialect) -> Self {
        Self {
            dialect: value,
            indentation_config: IndentationConfig::default(),
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: IndentationConfig) -> Self {
        Self {
            dialect,
            indentation_config,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        self.dialect
    }

    pub fn indentation_config(&self) -> IndentationConfig {
        self.indentation_config
    }

    pub fn parse(
        &self,
        tables: &Tables,
        segments: &[ErasedSegment],
    ) -> Result<Option<ErasedSegment>, SQLParseError> {
        if segments.is_empty() {
            // This should normally never happen because there will usually
            // be an end_of_file segment. It would probably only happen in
            // api use cases.
            return Ok(None);
        }

        let compiled = self
            .dialect
            .compile_grammar()
            .map_err(|err| SQLParseError {
                description: format!("Failed to compile grammar: {err}"),
                segment: None,
            })?;
        let root = compiled.root_parse_file(
            tables,
            self.dialect.name(),
            self.dialect,
            segments,
            self.indentation_config,
        )?;

        #[cfg(debug_assertions)]
        {
            let join_segments_raw = |segments: &[ErasedSegment]| {
                smol_str::SmolStr::from_iter(segments.iter().map(|s| s.raw().as_str()))
            };

            pretty_assertions::assert_eq!(&join_segments_raw(segments), root.raw());
        }

        Ok(root.into())
    }
}
