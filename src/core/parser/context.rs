use std::collections::HashMap;

#[derive(Debug)]
pub struct RootParseContext {
    dialect: Dialect,
    recurse: bool,
    indentation_config: HashMap<String, bool>,
    denylist: ParseDenylist,
    logger: Logger,
    uuid: uuid::Uuid,
}

impl RootParseContext {
    pub fn from_config(
        config: FluffConfig,
        overrides: HashMap<String, bool>,
    ) -> Result<RootParseContext, String> {
        let indentation_config = config.get_section("indentation").unwrap_or(HashMap::new());
        let indentation_config = indentation_config
            .iter()
            .map(|(k, v)| {
                let v = match v {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("Invalid indentation config: {}={}", k, v)),
                };
                Ok((k, v))
            })
            .collect::<Result<HashMap<String, bool>, String>>()?;
        let ctx = RootParseContext {
            dialect: config.get("dialect_obj").unwrap(),
            recurse: config.get("recurse").unwrap(),
            indentation_config,
            denylist: ParseDenylist::new(),
            logger: parser_logger(),
            uuid: uuid::Uuid::new_v4(),
        };
        for (key, value) in overrides {
            if let Some(value) = value {
                ctx.key = value;
            }
        }
        Ok(ctx)
    }

    pub fn enter(&self) -> Result<ParseContext, String> {
        ParseContext::new(self, self.recurse)
    }
}

#[derive(Debug)]
struct ParseContext {
    match_depth: u32,
    parse_depth: u32,
    match_segment: String,
    recurse: bool,
    _root_ctx: ParseContext,
}

impl ParseContext {
    fn __init__(root_ctx: ParseContext, recurse: bool) -> Self {
        ParseContext {
            _root_ctx: root_ctx,
            recurse: recurse,
            match_segment: String::new(),
            match_depth: 0,
            parse_depth: 0,
        }
    }

    fn __getattr__(name: String) -> String {
        try {
            return getattr(self._root_ctx, name);
        } catch AttributeError:  # pragma: no cover
            raise AttributeError(
                "Attribute {!r} not found in {!r} or {!r}".format(
                    name, type(self).__name__, type(self._root_ctx).__name__
                )
            )
    }

    fn _copy() -> Self {
        """Mimic the copy.copy() method but restrict only to local vars."""
        ctx = self.__class__(root_ctx=self._root_ctx)
        for key in self.__slots__:
            setattr(ctx, key, getattr(self, key))
        return ctx
    }

    fn __enter__() -> Self {
        """Enter into the context.
        For the ParseContext, this just returns itself, because
        we already have the right kind of object.
        """
        return self
    }

    fn __exit__(type: u32, value: u32, traceback: u32) -> bool {
        """Clear up the context."""
        return false;
    }

    fn deeper_match() -> Self {
        """Return a copy with an incremented match depth."""
        ctx = self._copy()
        ctx.match_depth += 1
        return ctx
    }

    fn deeper_parse() -> Self {
        """Return a copy with an incremented parse depth."""
        ctx = self._copy()
        if not isinstance(ctx.recurse, bool):  # pragma: no cover TODO?
            ctx.recurse -= 1
        ctx.parse_depth += 1
        ctx.match_depth = 0
        return ctx
    }

    fn may_recurse() -> bool {
        """Return True if allowed to recurse."""
        return self.recurse > 1 || self.recurse is true
    }

    fn matching_segment(name: String) -> Self {
        """Set the name of the current matching segment.
        NB: We don't reset the match depth here.
        """
        ctx = self._copy()
        ctx.match_segment = name
        return ctx
    }
}

struct ParseDenylist {
    _denylist_struct: HashMap<String, HashSet<String>>,
}

impl ParseDenylist {
    fn __init__() -> Self {
        ParseDenylist {
            _denylist_struct: HashMap::new(),
        }
    }
}
