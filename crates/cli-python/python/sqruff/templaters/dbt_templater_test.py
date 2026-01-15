import json
import os
from pathlib import Path

from sqruff.templaters.dbt_templater import (
    _get_or_create_templater,
    _templater_cache,
    clear_templater_cache,
    process_batch_from_rust,
    process_from_rust,
)
from sqruff.templaters.python_templater import FluffConfig


def test_dbt():
    # Use PROJECT_ROOT env var if set (for Bazel sandbox), otherwise use __file__
    if "PROJECT_ROOT" in os.environ:
        project_root = Path(os.environ["PROJECT_ROOT"])
        folder = project_root / "crates/cli-python/tests/dbt_sample"
    else:
        current = Path(os.path.dirname(os.path.abspath(__file__)))
        folder = current.joinpath("../../../tests/dbt_sample")
    file = folder.joinpath("models/customers.sql")
    profiles = folder.joinpath("profiles")

    templated_file = process_from_rust(
        """
{{ config(materialized='table') }}

with source_data as (

    select 1 as id
    union all
    select null as id

)

select *
from source_data
        """,
        str(file.resolve()),
        json.dumps(
            FluffConfig(
                templater_unwrap_wrapped_queries=False,
                jinja_apply_dbt_builtins=True,
                jinja_library_paths=None,
                jinja_templater_paths=None,
                jinja_loader_search_path=None,
                jinja_ignore_templating=None,
                dbt_target=None,
                dbt_profile=None,
                dbt_target_path=None,
                dbt_context=None,
                dbt_project_dir=str(folder),
                dbt_profiles_dir=str(profiles),
            )._asdict()
        ),
        {},
    )

    assert templated_file.sliced_file
    assert len(templated_file.sliced_file) > 1
    assert templated_file.raw_sliced
    assert len(templated_file.raw_sliced) > 1


def test_templater_caching():
    """Test that templater instances are cached per project directory."""
    current = Path(os.path.dirname(os.path.abspath(__file__)))
    folder = current.joinpath("../../../tests/dbt_sample")
    profiles = folder.joinpath("profiles")

    # Clear the cache first
    clear_templater_cache()
    assert len(_templater_cache) == 0

    config = FluffConfig(
        templater_unwrap_wrapped_queries=False,
        jinja_apply_dbt_builtins=True,
        jinja_library_paths=None,
        jinja_templater_paths=None,
        jinja_loader_search_path=None,
        jinja_ignore_templating=None,
        dbt_target=None,
        dbt_profile=None,
        dbt_target_path=None,
        dbt_context=None,
        dbt_project_dir=str(folder),
        dbt_profiles_dir=str(profiles),
    )
    live_context = {}

    # Get a templater - should create a new one
    templater1 = _get_or_create_templater(config, live_context)
    assert len(_templater_cache) == 1

    # Get another templater for the same project - should reuse
    templater2 = _get_or_create_templater(config, live_context)
    assert len(_templater_cache) == 1
    assert templater1 is templater2  # Same instance

    # Clear and verify
    clear_templater_cache()
    assert len(_templater_cache) == 0


def test_batch_processing():
    """Test batch processing of multiple files."""
    current = Path(os.path.dirname(os.path.abspath(__file__)))
    folder = current.joinpath("../../../tests/dbt_sample")
    profiles = folder.joinpath("profiles")

    # Clear the cache first
    clear_templater_cache()

    config_dict = FluffConfig(
        templater_unwrap_wrapped_queries=False,
        jinja_apply_dbt_builtins=True,
        jinja_library_paths=None,
        jinja_templater_paths=None,
        jinja_loader_search_path=None,
        jinja_ignore_templating=None,
        dbt_target=None,
        dbt_profile=None,
        dbt_target_path=None,
        dbt_context=None,
        dbt_project_dir=str(folder),
        dbt_profiles_dir=str(profiles),
    )._asdict()
    config_string = json.dumps(config_dict)
    live_context = {}

    # Get all SQL files from the test project
    models_path = folder / "models"
    files = []
    for sql_file in models_path.rglob("*.sql"):
        content = sql_file.read_text()
        files.append((content, str(sql_file.resolve())))

    assert len(files) > 0, "Should have found SQL files"

    # Process all files in a batch
    results = process_batch_from_rust(files, config_string, live_context)

    # Verify results
    assert len(results) == len(files)

    success_count = 0
    for templated_file, error in results:
        if error is None:
            assert templated_file is not None
            success_count += 1
        else:
            # Some files might fail if they have dependencies we can't resolve
            print(f"File failed with error: {error}")

    # At least some files should succeed
    assert success_count > 0, "At least some files should be processed successfully"

    # Verify the cache was used (only one templater created)
    assert len(_templater_cache) == 1


def test_batch_processing_empty():
    """Test batch processing with empty input."""
    clear_templater_cache()
    results = process_batch_from_rust([], "{}", {})
    assert results == []


def test_batch_processing_preserves_order():
    """Test that batch processing returns results in the same order as input."""
    current = Path(os.path.dirname(os.path.abspath(__file__)))
    folder = current.joinpath("../../../tests/dbt_sample")
    profiles = folder.joinpath("profiles")

    clear_templater_cache()

    config_dict = FluffConfig(
        templater_unwrap_wrapped_queries=False,
        jinja_apply_dbt_builtins=True,
        jinja_library_paths=None,
        jinja_templater_paths=None,
        jinja_loader_search_path=None,
        jinja_ignore_templating=None,
        dbt_target=None,
        dbt_profile=None,
        dbt_target_path=None,
        dbt_context=None,
        dbt_project_dir=str(folder),
        dbt_profiles_dir=str(profiles),
    )._asdict()
    config_string = json.dumps(config_dict)
    live_context = {}

    # Get all SQL files from the test project
    models_path = folder / "models"
    files = []
    for sql_file in models_path.rglob("*.sql"):
        content = sql_file.read_text()
        files.append((content, str(sql_file.resolve())))

    if len(files) < 2:
        return  # Need at least 2 files for this test

    # Process all files in a batch
    results = process_batch_from_rust(files, config_string, live_context)

    # Results should be in the same order as input
    assert len(results) == len(files)

    # For each result, if successful, the fname should match
    for i, (templated_file, error) in enumerate(results):
        if templated_file is not None:
            expected_fname = files[i][1]
            # The templated file should be for the correct input file
            assert templated_file.fname == expected_fname, (
                f"Result {i} fname mismatch: expected {expected_fname}, "
                f"got {templated_file.fname}"
            )
