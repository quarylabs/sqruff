"""Tests for the sqruff Python API.

Requires the package to be built first:
    maturin develop
"""

import io
import json
import pathlib
import subprocess
import sys
import tempfile
import textwrap

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

    def test_from_file_with_name(self, tmp_path):
        path = tmp_path / ".sqruff"
        path.write_text(
            "[sqruff]\ndialect = snowflake\n"
            "[sqruff:templater:dbt]\nproject_dir = project\n",
        )
        with path.open() as file:
            config = FluffConfig.from_file(file)
        assert str(config.dialect) == "snowflake"
        assert config.get_templater_value("dbt", "project_dir") == str(
            tmp_path / "project",
        )

    def test_from_source_with_path(self, tmp_path):
        # The path supplies a base for relative values; it need not exist.
        config = FluffConfig.from_source(
            "[sqruff]\ndialect = snowflake\n"
            "[sqruff:templater:dbt]\nproject_dir = project\n",
            path=str(tmp_path / ".sqruff"),
        )
        assert str(config.dialect) == "snowflake"
        assert config.get_templater_value("dbt", "project_dir") == str(
            tmp_path / "project",
        )

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
    @pytest.mark.parametrize("ignore_templating", [False, True])
    def test_jinja_cli_uses_file_config_and_reports_templating_errors(
        self, tmp_path, ignore_templating
    ):
        config = "[sqruff]\ndialect = ansi\ntemplater = jinja\n"
        if ignore_templating:
            config += "ignore = templating\n"
        (tmp_path / ".sqruff").write_text(config)
        (tmp_path / "ign.sql").write_text("SELECT {{ undefined_macro('x') }} AS y\n")

        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "from sqruff.main import main; main()",
                "lint",
                "--format",
                "json",
                "ign.sql",
            ],
            cwd=tmp_path,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )

        assert result.returncode == (0 if ignore_templating else 1), result.stderr
        files = json.loads(result.stdout)
        assert len(files) == 1
        filename, violations = next(iter(files.items()))
        assert pathlib.Path(filename).name == "ign.sql"
        if ignore_templating:
            assert violations == []
        else:
            assert len(violations) == 1
            assert violations[0]["code"] == "TMP"
            assert (
                "Undefined jinja template variable: 'undefined_macro'"
                in violations[0]["message"]
            )

    @pytest.mark.parametrize(
        "extra_config",
        [
            "ignore = templating\n",
            "ignore = parsing, templating\n",
            "[sqruff:templater:jinja]\nignore_templating = True\n",
            (
                "ignore = templating\n"
                "[sqruff:templater:jinja]\nignore_templating = False\n"
            ),
        ],
    )
    def test_jinja_ignore_templating(self, extra_config):
        config = FluffConfig.from_source(
            "[sqruff]\ndialect = ansi\ntemplater = jinja\n" + extra_config
        )
        result = Linter(config).lint_string("SELECT {{ undefined_macro('x') }} AS y\n")

        assert not result.has_violations

    @pytest.mark.parametrize("extra_config", ["", "ignore = parsing\n"])
    def test_jinja_undefined_macro_reports_its_name(self, extra_config):
        config = FluffConfig.from_source(
            "[sqruff]\ndialect = ansi\ntemplater = jinja\n" + extra_config
        )
        with pytest.raises(
            RuntimeError, match="Undefined jinja template variable: 'undefined_macro'"
        ):
            Linter(config).lint_string("SELECT {{ undefined_macro('x') }} AS y\n")

    @pytest.mark.parametrize("method", ["lint_string", "lint_paths"])
    def test_shared_linter_does_not_deadlock(self, method, tmp_path):
        sql_path = tmp_path / "test.sql"
        sql_path.write_text("SELECT 1\n")
        # Run in a subprocess so a GIL/mutex deadlock cannot hang pytest itself.
        script = textwrap.dedent(
            """
            import sys
            from concurrent.futures import ThreadPoolExecutor
            from threading import Event, Timer

            import sqruff
            import sqruff.templaters.jinja_templater as jinja

            entered = Event()
            proceed = Event()
            original = jinja.process_variants_from_rust

            def process(*args):
                if not entered.is_set():
                    entered.set()
                    assert proceed.wait(timeout=5)
                return original(*args)

            jinja.process_variants_from_rust = process
            config = sqruff.FluffConfig.from_source(
                '[sqruff]\\ntemplater = jinja\\nrules = \\n'
            )
            linter = sqruff.Linter(config)

            def lint():
                if sys.argv[1] == 'lint_string':
                    return linter.lint_string('SELECT 1\\n')
                return linter.lint_paths([sys.argv[2]])

            with ThreadPoolExecutor(max_workers=1) as pool:
                first = pool.submit(lint)
                assert entered.wait(timeout=5)
                # This timer must acquire the GIL while the second call waits
                # for the mutex held by the first call.
                timer = Timer(0.1, proceed.set)
                timer.start()
                try:
                    assert not lint().has_violations
                    assert not first.result(timeout=5).has_violations
                finally:
                    proceed.set()
                    timer.join()
            """,
        )
        result = subprocess.run(
            [sys.executable, "-c", script, method, str(sql_path)],
            check=False,
            capture_output=True,
            text=True,
            timeout=20,
        )
        assert result.returncode == 0, result.stderr

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


class TestTemplateFixability:
    CONFIG = """[sqruff]
dialect = ansi
templater = jinja
rules = ST05
[sqruff:templater:jinja:context]
tbl = my_table
[sqruff:rules:structure.subquery]
forbid_subquery_in = join
"""
    PLAIN_SQL = (
        "select a.id, b.name\n"
        "from foo as a\n"
        "inner join (select id, name from bar) as b on a.id = b.id\n"
    )
    TEMPLATED_SQL = PLAIN_SQL.replace("from bar", "from {{ tbl }}")

    @pytest.mark.parametrize("fix", [False, True])
    @pytest.mark.parametrize(
        "source",
        [
            TEMPLATED_SQL,
            PLAIN_SQL.replace("from foo", "from {{ tbl }}"),
            PLAIN_SQL.replace("select a.id, b.name", "select {{ 'a.id' }}, b.name"),
        ],
    )
    def test_discarded_template_fixes_are_unfixable(self, source, fix):
        linter = Linter(FluffConfig.from_source(self.CONFIG))
        result = linter.lint_string(source, fix=fix)

        assert [(v.code, v.fixable) for v in result.violations] == [("ST05", False)]
        assert result.has_unfixable_violations
        assert not result.has_fixes
        assert result.fix_string() == source

    @pytest.mark.parametrize("fix", [False, True])
    def test_safe_subquery_fixes_remain_fixable(self, fix):
        linter = Linter(FluffConfig.from_source(self.CONFIG))
        result = linter.lint_string(self.PLAIN_SQL, fix=fix)

        assert [(v.code, v.fixable) for v in result.violations] == [("ST05", True)]
        assert not result.has_unfixable_violations
        assert result.has_fixes == fix
        fixed = result.fix_string()
        if fix:
            assert fixed != self.PLAIN_SQL
            assert not linter.lint_string(fixed).has_violations
        else:
            assert fixed == self.PLAIN_SQL

    @pytest.mark.parametrize("use_stdin", [False, True])
    @pytest.mark.parametrize("templated", [False, True])
    def test_fix_exit_status_matches_applicable_fixes(
        self, tmp_path, templated, use_stdin
    ):
        config_path = tmp_path / ".sqruff"
        config_path.write_text(self.CONFIG)
        sql_path = tmp_path / "query.sql"
        source = self.TEMPLATED_SQL if templated else self.PLAIN_SQL
        sql_path.write_text(source)

        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "from sqruff.main import main; main()",
                "--config",
                str(config_path),
                "fix",
                "-" if use_stdin else str(sql_path),
            ],
            input=source if use_stdin else None,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )

        assert result.returncode == (1 if templated else 0), result.stderr
        fixed = result.stdout if use_stdin else sql_path.read_text()
        if templated:
            assert "ST05" in result.stderr
            assert fixed == source
        else:
            assert fixed != source
            linter = Linter(FluffConfig.from_source(self.CONFIG))
            assert not linter.lint_string(fixed).has_violations


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
