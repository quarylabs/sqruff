try:
    from ._sqruff import (  # noqa
        run_cli,
        Violation,
        DialectKind,
        RuleGroups,
        PlaceholderStyle,
        TemplaterKind,
        FluffConfig,
        LintedFile,
        LintingResult,
        Linter,
    )
except ImportError:
    pass


__all__ = (
    "DialectKind",
    "FluffConfig",
    "LintedFile",
    "Linter",
    "LintingResult",
    "PlaceholderStyle",
    "RuleGroups",
    "TemplaterKind",
    "Violation",
    "fix",
    "lint",
    "run_cli",
)


def lint(sql: str, *, config: "FluffConfig | None" = None) -> "LintedFile":
    """Lint (and optionally fix) a SQL string.

    Args:
        sql: The SQL string to lint.
        config: Optional FluffConfig. If not provided, a default config is used.

    Returns:
        A LintedFile. Call .fix_string() for the fixed SQL, .violations for what was found.
    """
    if config is None:
        config = FluffConfig()
    linter = Linter(config)
    return linter.lint_string(sql, fix=True)


def fix(sql: str, *, config: "FluffConfig | None" = None) -> str:
    """Fix (format) a SQL string and return the result.

    Args:
        sql: The SQL string to fix.
        config: Optional FluffConfig. If not provided, a default config is used.

    Returns:
        The fixed SQL string.
    """
    return lint(sql, config=config).fix_string()
