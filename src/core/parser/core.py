
class RootParseContext:
    """Object to handle the context at hand during parsing.
    The root context holds the persistent config which stays
    consistent through a parsing operation. It also produces
    the individual contexts that are used at different layers.
    Each ParseContext maintains a reference to the RootParseContext
    which created it so that it can refer to config within it.
    """

    def __init__(self, dialect, indentation_config=None, recurse=True):
        """Store persistent config objects."""
        self.dialect = dialect
        self.recurse = recurse
        # Indentation config is used by Indent and Dedent and used to control
        # the intended indentation of certain features. Specifically it is
        # used in the Conditional grammar.
        self.indentation_config = indentation_config or {}
        # Initialise the denylist
        self.denylist = ParseDenylist()
        # This is the logger that child objects will latch onto.
        self.logger = parser_logger
        # A uuid for this parse context to enable cache invalidation
        self.uuid = uuid.uuid4()

    @classmethod
    def from_config(cls, config, **overrides: Dict[str, bool]) -> "RootParseContext":
        """Construct a `RootParseContext` from a `FluffConfig`."""
        indentation_config = config.get_section("indentation") or {}
        try:
            indentation_config = {k: bool(v) for k, v in indentation_config.items()}
        except TypeError:  # pragma: no cover
            raise TypeError(
                "One of the configuration keys in the `indentation` section is not "
                "True or False: {!r}".format(indentation_config)
            )
        ctx = cls(
            dialect=config.get("dialect_obj"),
            recurse=config.get("recurse"),
            indentation_config=indentation_config,
        )
        # Set any overrides in the creation
        for key in overrides:
            if overrides[key] is not None:
                setattr(ctx, key, overrides[key])
        return ctx

    def __enter__(self):
        """Enter into the context.
        Here we return a basic ParseContext with initial values,
        initialising just the recurse value.
        Note: The RootParseContext is usually entered at the beginning
        of the parse operation as follows::
            with RootParseContext.from_config(...) as ctx:
                parsed = file_segment.parse(parse_context=ctx)
        """
        return ParseContext(root_ctx=self, recurse=self.recurse)

    def __exit__(self, type, value, traceback):
        """Clear up the context."""
        pass