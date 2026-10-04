"""SQLMesh templater source mapping and sqruff bridge tests."""

import json
from pathlib import Path

import pytest

from sqruff.templaters.python_templater import FluffConfig
from sqruff.templaters.sqlmesh_templater import (
    SQLMeshTemplater,
    process_batch_from_rust,
)

PROJECT = Path(__file__).resolve().parents[3] / "tests" / "sqlmesh_sample"


def _config() -> FluffConfig:
    return FluffConfig(
        templater_unwrap_wrapped_queries=False,
        jinja_templater_paths=[],
        jinja_exclude_macros_from_path=[],
        jinja_loader_search_path=[],
        jinja_apply_dbt_builtins=False,
        jinja_ignore_templating=False,
        jinja_library_paths=[],
        dbt_profile=None,
        dbt_profiles_dir=None,
        dbt_target=None,
        dbt_target_path=None,
        dbt_context=None,
        dbt_project_dir=None,
        sqlmesh_project_dir=str(PROJECT),
        sqlmesh_config="config",
        sqlmesh_gateway="local",
        sqlmesh_dialect="duckdb",
    )


def test_plain_model_keeps_exact_literal_positions(monkeypatch):
    source = (PROJECT / "models" / "simple_model.sql").read_text()
    templater = SQLMeshTemplater()

    def fail_if_context_loaded(*_args):
        raise AssertionError("Plain SQL must not load SQLMesh")

    monkeypatch.setattr(templater, "_resolve_model", fail_if_context_loaded)
    output, errors = templater.process(
        fname=str(PROJECT / "models" / "simple_model.sql"),
        in_str=source,
        config=_config(),
    )

    assert not errors
    assert output.templated_str.strip().startswith("SELECT")
    assert "MODEL (" not in output.templated_str
    assert [part.slice_type for part in output.sliced_file] == [
        "templated",
        "literal",
    ]
    assert output.sliced_file[0].templated_slice == slice(0, 0)
    assert source[output.sliced_file[1].source_slice] == output.templated_str


def test_macro_scanner_ignores_strings_and_comments():
    source = "SELECT '@email', @start_date -- @comment\n/* @block */"
    spans = SQLMeshTemplater._find_macro_spans(source)
    assert [source[start:end] for start, end in spans] == ["@start_date"]


def test_batch_bridge_preserves_order_and_positions():
    source = (PROJECT / "models" / "simple_model.sql").read_text()
    config = json.dumps(_config()._asdict())
    results = process_batch_from_rust(
        [(source, str(PROJECT / "models" / "simple_model.sql")), ("SELECT 2", "stdin")],
        config,
    )

    assert len(results) == 2
    assert all(error is None for _, error in results)
    assert results[0][0].templated_str.strip().startswith("SELECT")
    assert results[1][0].templated_str == "SELECT 2"


def test_sqlmesh_macro_model_uses_project_context():
    pytest.importorskip("sqlmesh")
    path = PROJECT / "models" / "model_with_macros.sql"
    output, errors = SQLMeshTemplater().process(
        fname=str(path), in_str=path.read_text(), config=_config()
    )

    assert not errors
    assert "MODEL (" not in output.templated_str
    assert "SELECT" in output.templated_str.upper()
    assert output.sliced_file
    assert output.raw_sliced
    assert [part.slice_type for part in output.sliced_file] == [
        "templated",
        "templated",
    ]


@pytest.mark.parametrize("model", ["incremental_model.sql", "python_macro_model.sql"])
def test_sqlmesh_inline_macros_preserve_literal_regions(model):
    pytest.importorskip("sqlmesh")
    path = PROJECT / "models" / model
    output, errors = SQLMeshTemplater().process(
        fname=str(path), in_str=path.read_text(), config=_config()
    )

    assert not errors
    assert "literal" in [part.slice_type for part in output.sliced_file]
    assert "templated" in [part.slice_type for part in output.sliced_file]


def test_sqlmesh_jinja_model_renders_without_source_collapse():
    pytest.importorskip("sqlmesh")
    path = PROJECT / "models" / "jinja_model.sql"
    source = path.read_text()
    output, errors = SQLMeshTemplater().process(
        fname=str(path), in_str=source, config=_config()
    )

    assert not errors
    assert output.templated_str
    assert output.sliced_file
    assert output.sliced_file[-1].source_slice.stop == len(source)
