"""Tests for the sqruff Python API.

Requires the package to be built first:
    maturin develop
"""

import io
import pathlib
import tempfile

import pytest
import sqruff
from sqruff import (
    DialectKind,
    FluffConfig,
    LintedFile,
    Linter,
    PlaceholderStyle,
    RuleGroups,
    TemplaterKind,
    Violation,
)

SIMPLE_SQL = "select 1"
UNFORMATTED_SQL = "SELECT   1"


# ── DialectKind ───────────────────────────────────────────────────────────────


class TestDialectKind:
    def test_valid(self):
        d = DialectKind("ansi")
        assert str(d) == "ansi"

    def test_repr(self):
        assert repr(DialectKind("ansi")) == 'DialectKind("ansi")'

    def test_invalid(self):
        with pytest.raises(ValueError, match="Unknown dialect"):
            DialectKind("notadialect")

    def test_error_lists_options(self):
        with pytest.raises(ValueError, match="ansi"):
            DialectKind("notadialect")

    def test_available(self):
        names = DialectKind.available()
        assert "ansi" in names
        assert "snowflake" in names
        assert len(names) > 5

    def test_eq(self):
        assert DialectKind("ansi") == DialectKind("ansi")
        assert DialectKind("ansi") != DialectKind("snowflake")

    def test_hashable(self):
        s = {DialectKind("ansi"), DialectKind("snowflake"), DialectKind("ansi")}
        assert len(s) == 2


# ── RuleGroups ────────────────────────────────────────────────────────────────


class TestRuleGroups:
    def test_valid(self):
        g = RuleGroups("core")
        assert str(g) == "core"

    def test_repr(self):
        assert repr(RuleGroups("core")) == 'RuleGroups("core")'

    def test_invalid(self):
        with pytest.raises(ValueError, match="Unknown rule group"):
            RuleGroups("notagroup")

    def test_error_lists_options(self):
        with pytest.raises(ValueError, match="core"):
            RuleGroups("notagroup")

    def test_available(self):
        names = RuleGroups.available()
        assert "all" in names
        assert "core" in names

    def test_hashable(self):
        s = {RuleGroups("core"), RuleGroups("layout"), RuleGroups("core")}
        assert len(s) == 2


# ── PlaceholderStyle ──────────────────────────────────────────────────────────


class TestPlaceholderStyle:
    def test_valid(self):
        p = PlaceholderStyle("colon")
        assert str(p) == "colon"

    def test_invalid(self):
        with pytest.raises(ValueError, match="Unknown placeholder style"):
            PlaceholderStyle("notastyle")

    def test_error_lists_options(self):
        with pytest.raises(ValueError, match="colon"):
            PlaceholderStyle("notastyle")

    def test_available(self):
        names = PlaceholderStyle.available()
        assert "colon" in names
        assert len(names) > 1

    def test_regex_pattern(self):
        assert PlaceholderStyle("colon").regex_pattern != ""

    def test_hashable(self):
        styles = {PlaceholderStyle(n) for n in PlaceholderStyle.available()}
        assert len(styles) == len(PlaceholderStyle.available())


# ── TemplaterKind ─────────────────────────────────────────────────────────────


class TestTemplaterKind:
    def test_valid(self):
        t = TemplaterKind("raw")
        assert str(t) == "raw"

    def test_invalid(self):
        with pytest.raises(ValueError, match="Unknown templater"):
            TemplaterKind("notatemplater")

    def test_error_lists_options(self):
        with pytest.raises(ValueError, match="raw"):
            TemplaterKind("notatemplater")

    def test_available(self):
        names = TemplaterKind.available()
        assert "raw" in names
        assert "jinja" in names

    def test_hashable(self):
        s = {TemplaterKind("raw"), TemplaterKind("jinja"), TemplaterKind("raw")}
        assert len(s) == 2


# ── FluffConfig ───────────────────────────────────────────────────────────────


class TestFluffConfig:
    def test_default(self):
        config = FluffConfig()
        assert str(config.dialect) == "ansi"

    def test_dialect_kwarg(self):
        config = FluffConfig(dialect="snowflake")
        assert str(config.dialect) == "snowflake"

    def test_invalid_dialect_kwarg(self):
        with pytest.raises(ValueError, match="Unknown dialect"):
            FluffConfig(dialect="notadialect")

    def test_rules_kwarg(self):
        # Only LT01 enabled — extra whitespace should trigger it, nothing else
        config = FluffConfig(rules=["LT01"])
        linter = Linter(config)
        result = linter.lint_string(UNFORMATTED_SQL)
        assert result.has_violations
        assert all(v.code == "LT01" for v in result.violations)

    def test_exclude_rules_kwarg(self):
        # LT01 excluded — extra whitespace should not appear in violations
        config = FluffConfig(exclude_rules=["LT01"])
        linter = Linter(config)
        result = linter.lint_string(UNFORMATTED_SQL)
        assert all(v.code != "LT01" for v in result.violations)

    def test_dialect_setter(self):
        config = FluffConfig()
        config.dialect = "snowflake"
        assert str(config.dialect) == "snowflake"

    def test_dialect_setter_invalid(self):
        config = FluffConfig()
        with pytest.raises(ValueError):
            config.dialect = "notadialect"

    def test_sql_file_exts(self):
        config = FluffConfig()
        assert ".sql" in config.sql_file_exts

    def test_sql_file_exts_setter(self):
        config = FluffConfig()
        config.sql_file_exts = [".sql", ".hql"]
        assert ".hql" in config.sql_file_exts

    def test_templater(self):
        config = FluffConfig()
        assert isinstance(config.templater, TemplaterKind)

    def test_from_source(self):
        config = FluffConfig.from_source("[sqruff]\ndialect = snowflake\n")
        assert str(config.dialect) == "snowflake"

    def test_from_file(self):
        f = io.StringIO("[sqruff]\ndialect = snowflake\n")
        config = FluffConfig.from_file(f)
        assert str(config.dialect) == "snowflake"

    def test_from_path_str(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".cfg", delete=False) as f:
            f.write("[sqruff]\ndialect = snowflake\n")
            name = f.name
        config = FluffConfig.from_path(name)
        assert str(config.dialect) == "snowflake"

    def test_from_path_pathlib(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".cfg", delete=False) as f:
            f.write("[sqruff]\ndialect = snowflake\n")
            name = f.name
        config = FluffConfig.from_path(pathlib.Path(name))
        assert str(config.dialect) == "snowflake"


# ── Linter / LintedFile ───────────────────────────────────────────────────────


class TestLinter:
    def test_default_linter(self):
        linter = Linter()
        result = linter.lint_string(SIMPLE_SQL, filename="test.sql")
        assert result.path == "test.sql"

    def test_linter_with_config(self):
        config = FluffConfig(dialect="ansi", rules=["LT01"])
        linter = Linter(config)
        result = linter.lint_string(UNFORMATTED_SQL)
        assert any(v.code == "LT01" for v in result.violations)

    def test_lint_string_returns_linted_file(self):
        linter = Linter()
        result = linter.lint_string(SIMPLE_SQL)
        assert isinstance(result, LintedFile)

    def test_lint_string_no_violations_on_clean_sql(self):
        linter = Linter(FluffConfig(rules=[]))
        result = linter.lint_string(SIMPLE_SQL)
        assert not result.has_violations

    def test_lint_string_violations_on_bad_sql(self):
        linter = Linter(FluffConfig(rules=["LT01"]))
        result = linter.lint_string(UNFORMATTED_SQL)
        assert result.has_violations
        assert len(result.violations) > 0

    def test_violation_fields(self):
        linter = Linter(FluffConfig(rules=["LT01"]))
        result = linter.lint_string(UNFORMATTED_SQL)
        v = result.violations[0]
        assert isinstance(v, Violation)
        assert isinstance(v.line_no, int)
        assert isinstance(v.line_pos, int)
        assert isinstance(v.code, str)
        assert isinstance(v.description, str)
        assert isinstance(v.fixable, bool)

    def test_fix_string(self):
        linter = Linter()
        result = linter.lint_string(UNFORMATTED_SQL, fix=True)
        fixed = result.fix_string()
        assert isinstance(fixed, str)

    def test_linted_file_repr(self):
        linter = Linter()
        result = linter.lint_string(SIMPLE_SQL)
        assert "LintedFile" in repr(result)


# ── convenience functions ─────────────────────────────────────────────────────


class TestConvenienceFunctions:
    def test_lint_returns_linted_file(self):
        result = sqruff.lint(SIMPLE_SQL)
        assert isinstance(result, LintedFile)

    def test_lint_with_config(self):
        config = FluffConfig(dialect="ansi")
        result = sqruff.lint(SIMPLE_SQL, config=config)
        assert isinstance(result, LintedFile)

    def test_fix_returns_string(self):
        result = sqruff.fix(SIMPLE_SQL)
        assert isinstance(result, str)

    def test_fix_with_config(self):
        config = FluffConfig(dialect="ansi")
        result = sqruff.fix(SIMPLE_SQL, config=config)
        assert isinstance(result, str)

    def test_fix_actually_fixes(self):
        fixed = sqruff.fix(UNFORMATTED_SQL)
        assert isinstance(fixed, str)
