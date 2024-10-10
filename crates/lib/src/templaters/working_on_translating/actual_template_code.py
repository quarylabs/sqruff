import os
from typing import (Dict, Optional, Any, Callable, Set, Iterable, Iterator, Tuple, List, Union, Type)
import jinja2
from jinja2 import (
    Environment,
    FileSystemLoader,
    TemplateError,
    TemplateSyntaxError,
    meta, TemplateNotFound,
)
from jinja2.ext import Extension
from jinja2.sandbox import SandboxedEnvironment

type FluffConfig = Dict[str, str]
# Templated File
type TemplatedFile = Tuple[str, str, str, str, str]


def process(in_str: str, f_name: str, config: FluffConfig) -> Tuple[TemplatedFile, List[SQLTemplaterError]]:
    env, live_context, render_func = construct_render_func(f_name=f_name, config=config)

    # Attempt to identify any undeclared variables or syntax errors.
    # The majority of variables will be found during the _crawl_tree
    # step rather than this first Exception which serves only to catch
    # catastrophic errors.
    try:
        syntax_tree = env.parse(in_str)
        potentially_undefined_variables = meta.find_undeclared_variables(
            syntax_tree
        )
    except Exception as err:
        templater_error = SQLTemplaterError(
            "Failed to parse Jinja syntax. Correct the syntax or select an "
            "alternative templater. Error: " + str(err)
        )
        # Capture a line number if we can.
        if isinstance(err, TemplateSyntaxError):
            templater_error.line_no = err.lineno
        raise templater_error

    undefined_variables = init_undefined_tracking(
        live_context,
        potentially_undefined_variables,
        # TODO This will need to be changed
        ignore_templating=("templating" in config.get("ignore")),
    )

    try:
        # Slice the file once rendered.
        raw_sliced, sliced_file, out_str = slice_file(
            in_str,
            render_func=render_func,
            config=config,
        )
        return (
            TemplatedFile(
                source_str=in_str,
                templated_str=out_str,
                fname=fname,
                sliced_file=sliced_file,
                raw_sliced=raw_sliced,
            ),
            self._generate_violations_for_undefined_variables(
                in_str, syntax_tree, undefined_variables
            ),
        )
    except (TemplateError, TypeError) as err:
        raise SQLTemplaterError(
            (
                "Unrecoverable failure in Jinja templating: {}. Have you "
                "correctly configured your variables? "
                "https://docs.sqlfluff.com/en/latest/perma/variables.html"
            ).format(err),
            # We don't have actual line number information, but specify
            # line 1 so users can ignore with "noqa" if they want. (The
            # default is line 0, which can't be ignored because it's not
            # a valid line number.)
            line_no=1,
            line_pos=1,
        )


def init_undefined_tracking(
        live_context: Dict[str, Any],
        potentially_undefined_variables: Iterable[str],
        ignore_templating: bool = False,
) -> Set[str]:
    """Sets up tracing of undefined template variables.

    NOTE: This works by mutating the `live_context` which
    is being used by the environment.
    """
    # NOTE: This set is modified by the `UndefinedRecorder` when run.
    undefined_variables: Set[str] = set()

    for val in potentially_undefined_variables:
        if val not in live_context:
            if ignore_templating:
                live_context[val] = DummyUndefined.create(val)
            else:
                live_context[val] = UndefinedRecorder(val, undefined_variables)

    return undefined_variables


def construct_render_func(f_name: Optional[str] = None, config: Optional[FluffConfig] = None) -> Tuple[
    Environment, Dict[str, Any], Callable[[str], str]]:
    """Builds and returns objects needed to create and run templates.

    Args:
        f_name: The name of the file.
        config: The configuration settings.

    Returns:
        Tuple[Environment, dict, Callable[[str], str]]: A tuple
        containing the following:
            - env (Environment): An instance of the 'Environment' class.
            - live_context (dict): A dictionary containing the live context.
            - render_func (Callable[[str], str]): A callable function
            that is used to instantiate templates.
    """
    # Load the context
    env = get_jinja_env(config)
    live_context = get_env_context(f_name, config, env)

    def render_func(in_str: str) -> str:
        """Used by JinjaTracer to instantiate templates.

        This function is a closure capturing internal state from process().
        Note that creating templates involves quite a bit of state known to
        _this_ function but not to JinjaTracer.

        https://www.programiz.com/python-programming/closure
        """
        try:
            template = env.from_string(in_str, globals=live_context)
        except TemplateSyntaxError as err:  # pragma: no cover
            # NOTE: If the template fails to parse, then this clause
            # will be triggered. However in normal that should never
            # happen because the template should already have been
            # validated by the point this is called. Typically that
            # happens when searching for undefined variables.
            raise SQLTemplaterError(
                f"Late failure to parse jinja template: {err}.",
                line_no=err.lineno,
            )
        return template.render()

    return env, live_context, render_func


def get_env_context(
        f_name: Optional[str],
        config: Optional[FluffConfig],
        env: Environment,
) -> Dict[str, Any]:
    """Get the templating context from the config.

    NOTE: This closely mirrors the `get_context` method which we inherit from the
    python templater, but extends the signature. For that reason we define a new
    method here, which internally refers to `get_context`.

    Args:
        f_name (str, optional): The name of the file.
        config (dict, optional): The configuration.
        env: The Jinja Environment.

    Returns:
        dict: The templating context.
    """
    # Load the context
    live_context = get_context(f_name, config)

    # Apply dbt builtin functions if we're allowed.
    if config:
        # first make libraries available in the context
        # so they can be used by the macros too
        libraries = self._extract_libraries_from_config(config=config)
        live_context.update(libraries)

        jinja_filters = libraries.get("SQLFLUFF_JINJA_FILTERS")
        if jinja_filters:
            env.filters.update(jinja_filters)

    # Load macros from path (if applicable)
    if config:
        macros_path = self._get_macros_path(config, "load_macros_from_path")
        exclude_macros_path = self._get_macros_path(
            config, "exclude_macros_from_path"
        )
        if macros_path:
            live_context.update(
                self._extract_macros_from_path(
                    macros_path,
                    env=env,
                    ctx=live_context,
                    exclude_paths=exclude_macros_path,
                )
            )

        # Load config macros, these will take precedence over macros from the path
        live_context.update(
            self._extract_macros_from_config(
                config=config, env=env, ctx=live_context
            )
        )

    return live_context


def slice_file(
        raw_str: str,
        render_func: Callable[[str], str],
        append_to_templated: str = "",
) -> Tuple[List[RawFileSlice], List[TemplatedFileSlice], str]:
    """Slice the file to determine regions where we can fix.

    Args:
        raw_str (str): The raw string to be sliced.
        render_func (Callable[[str], str]): The rendering function to be used.
        append_to_templated: Optional string to append to the template.

    Returns:
        Tuple[List[RawFileSlice], List[TemplatedFileSlice], str]:
            A tuple containing a list of raw file slices, a list of
            templated file slices, and the templated string.
    """
    # The JinjaTracer slicing algorithm is more robust, but it requires
    # us to create and render a second template (not raw_str).

    analyzer = self._get_jinja_analyzer(raw_str, self._get_jinja_env())
    tracer = analyzer.analyze(render_func)
    trace = tracer.trace(append_to_templated=append_to_templated)
    return trace.raw_sliced, trace.sliced_file, trace.templated_str

def get_jinja_env(config: FluffConfig) -> Environment:
    """Get a properly configured jinja environment.

    This method returns a properly configured jinja environment. It
    first checks if the 'ignore' key is present in the config dictionary and
    if it contains the value 'templating'. If so, it creates a subclass of
    FileSystemLoader called SafeFileSystemLoader that overrides the
    get_source method to handle missing templates when templating is ignored.
    If 'ignore' is not present or does not contain 'templating', it uses the
    regular FileSystemLoader. It then sets the extensions to ['jinja2.ext.do']
    and adds the DBTTestExtension if the _apply_dbt_builtins method returns
    True. Finally, it returns a SandboxedEnvironment object with the
    specified settings.

    Args:
        config (dict, optional): A dictionary containing configuration settings.

    Returns:
        jinja2.Environment: A properly configured jinja environment.
    """
    loader: Optional[FileSystemLoader]
    macros_path = get_macros_path(config, "load_macros_from_path")
    loader_search_path = get_loader_search_path(config)
    final_search_path = (loader_search_path or []) + (macros_path or [])

    ignore_templating = config and "templating" in config.get("ignore")
    if ignore_templating:

        class SafeFileSystemLoader(FileSystemLoader):
            def get_source(
                    self, environment: Environment, name: str
            ) -> Tuple[str, str, Callable[..., Any]]:
                try:
                    if not isinstance(name, DummyUndefined):
                        return super().get_source(environment, name)
                    raise TemplateNotFound(str(name))
                except TemplateNotFound:
                    # When ignore=templating is set, treat missing files
                    # or attempts to load an "Undefined" file as the first
                    # 'base' part of the name / filename rather than failing.
                    templater_logger.debug(
                        "Providing dummy contents for Jinja macro file: %s", name
                    )
                    value = os.path.splitext(os.path.basename(str(name)))[0]
                    return value, f"{value}.sql", lambda: False

        loader = SafeFileSystemLoader(final_search_path or [])
    else:
        loader = FileSystemLoader(final_search_path) if final_search_path else None
    extensions: List[Union[str, Type[Extension]]] = ["jinja2.ext.do"]

    return SandboxedEnvironment(
        # We explicitly want to preserve newlines.
        keep_trailing_newline=True,
        # The do extension allows the "do" directive
        autoescape=False,
        extensions=extensions,
        loader=loader,
    )


def get_macros_path(config: Optional[FluffConfig], key: str) -> Optional[List[str]]:
    """Get the list of macros paths from the provided config object.

    This method searches for a config section specified by the
    templater_selector, name, and key specified. If the section is
    found, it retrieves the value associated with that section and splits it into
    a list of strings using a comma as the delimiter. The resulting list is
    stripped of whitespace and empty strings and returned. If the section is not
    found or the resulting list is empty, it returns None.

    Args:
        config (FluffConfig): The config object to search for the macros path
            section.
        key (str): Key to load the macros path from the config file.
            Also used for loading the excluding macros path from config.

    Returns:
        Optional[List[str]]: The list of macros paths if found, None otherwise.
    """
    if config:
        macros_path = config.get_section((self.templater_selector, self.name, key))
        if macros_path:
            result = [s.strip() for s in macros_path.split(",") if s.strip()]
            if result:
                return result
    return None


def get_loader_search_path(config: Optional[FluffConfig]) -> Optional[List[str]]:
    """Get the list of Jinja loader search paths from the provided config object.

    This method searches for a config section specified by the
    templater_selector, name, and 'loader_search_path' keys. If the section is
    found, it retrieves the value associated with that section and splits it into
    a list of strings using a comma as the delimiter. The resulting list is
    stripped of whitespace and empty strings and returned. If the section is not
    found or the resulting list is empty, it returns None.

    Args:
        config (FluffConfig): The config object to search for the loader search
            path section.

    Returns:
        Optional[List[str]]: The list of loader search paths if found, None
            otherwise.
    """
    if config:
        loader_search_path = config.get_section(
            (self.templater_selector, self.name, "loader_search_path")
        )
        if loader_search_path:
            result = [s.strip() for s in loader_search_path.split(",") if s.strip()]
            if result:
                return result
    return None


class UndefinedRecorder:
    """Similar to jinja2.StrictUndefined, but remembers, not fails."""

    # Tell Jinja this object is safe to call and does not alter data.
    # https://jinja.palletsprojects.com/en/2.9.x/sandbox/#jinja2.sandbox.SandboxedEnvironment.is_safe_callable
    unsafe_callable = False
    # https://jinja.palletsprojects.com/en/3.0.x/sandbox/#jinja2.sandbox.SandboxedEnvironment.is_safe_callable
    alters_data = False

    def __init__(self, name: str, undefined_set: Set[str]) -> None:
        self.name = name
        # Reference to undefined set to modify, it is assumed that the
        # calling code keeps a reference to this variable to they can
        # continue to access it after modification by this class.
        self.undefined_set = undefined_set

    def __str__(self) -> str:
        """Treat undefined vars as empty, but remember for later."""
        self.undefined_set.add(self.name)
        return ""

    def __getattr__(self, item: str) -> "UndefinedRecorder":
        """Don't fail when called, remember instead."""
        self.undefined_set.add(self.name)
        return UndefinedRecorder(f"{self.name}.{item}", self.undefined_set)

    def __call__(self, *args: Any, **kwargs: Any) -> "UndefinedRecorder":
        """Don't fail when called unlike parent class."""
        return UndefinedRecorder(f"{self.name}()", self.undefined_set)


class DummyUndefined(jinja2.Undefined):
    """Acts as a dummy value to try and avoid template failures.

    Inherits from jinja2.Undefined so Jinja's default() filter will
    treat it as a missing value, even though it has a non-empty value
    in normal contexts.
    """

    # Tell Jinja this object is safe to call and does not alter data.
    # https://jinja.palletsprojects.com/en/2.9.x/sandbox/#jinja2.sandbox.SandboxedEnvironment.is_safe_callable
    unsafe_callable = False
    # https://jinja.palletsprojects.com/en/3.0.x/sandbox/#jinja2.sandbox.SandboxedEnvironment.is_safe_callable
    alters_data = False

    def __init__(self, name: str) -> None:
        super().__init__()
        self.name = name

    def __str__(self) -> str:
        return self.name.replace(".", "_")

    @classmethod
    def create(cls, name: str) -> "DummyUndefined":
        """Factory method.

        When ignoring=templating is configured, use 'name' as the value for
        undefined variables. We deliberately avoid recording and reporting
        undefined variables as errors. Using 'name' as the value won't always
        work, but using 'name', combined with implementing the magic methods
        (such as __eq__, see above), works well in most cases.
        """
        result = DummyUndefined(name)
        return result

    def __getattr__(self, item: str) -> "DummyUndefined":
        """Intercept any calls to undefined attributes.

        Args:
            item (str): The name of the attribute.

        Returns:
            object: A dynamically created instance of this class.
        """
        return self.create(f"{self.name}.{item}")

    # Implement the most common magic methods. This helps avoid
    # templating errors for undefined variables.
    # https://www.tutorialsteacher.com/python/magic-methods-in-python
    def _self_impl(self, *args: Any, **kwargs: Any) -> "DummyUndefined":
        """Return an instance of the class itself.

        Args:
            *args: Variable length argument list.
            **kwargs: Arbitrary keyword arguments.

        Returns:
            object: An instance of the class itself.
        """
        return self

    def _bool_impl(self, *args: Any, **kwargs: Any) -> bool:
        """Return a boolean value.

        Args:
            *args: Variable length argument list.
            **kwargs: Arbitrary keyword arguments.

        Returns:
            bool: A boolean value.
        """
        return True

    # We're intentionally changing the behaviour here compared to the base
    # class of Undefined. That means we're going to silence the `assignment`
    # mypy warnings. Operations on an undefined result in another undefined.
    __add__ = __sub__ = __mul__ = _self_impl  # type: ignore[assignment]
    __floordiv__ = __truediv__ = _self_impl  # type: ignore[assignment]
    __mod__ = __pow__ = _self_impl  # type: ignore[assignment]
    __pos__ = __neg__ = _self_impl  # type: ignore[assignment]
    __lshift__ = __rshift__ = _self_impl
    __getitem__ = _self_impl  # type: ignore[assignment]
    __invert__ = _self_impl
    __call__ = _self_impl  # type: ignore[assignment]
    # Boolean operations on an undefined are handled separately.
    __and__ = __or__ = __xor__ = __bool__ = _bool_impl
    __lt__ = __le__ = __ge__ = __gt__ = _bool_impl  # type: ignore[assignment]
    __eq__ = __ne__ = _bool_impl

    def __hash__(self) -> int:  # pragma: no cover
        """Return a constant hash value.

        Returns:
            int: A constant hash value.
        """
        # This is called by the "in" operator, among other things.
        return 0

    def __iter__(self) -> Iterator["DummyUndefined"]:
        """Return an iterator that contains only the instance of the class itself.

        Returns:
            iterator: An iterator.
        """
        return [self].__iter__()
