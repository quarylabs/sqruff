import json
import os
import tempfile
from pathlib import Path

from sqruff.templaters.dbt_templater import process_from_rust
from sqruff.templaters.python_templater import FluffConfig


def test_dbt():
    current = Path(os.path.dirname(os.path.abspath(__file__)))
    folder = current.joinpath("../../../tests/dbt_sample")
    file = folder.joinpath("models/customers.sql")
    profiles = folder.joinpath("profiles")

    # Use absolute paths without resolving symlinks (needed for Bazel sandbox)
    folder_abs = str(folder.absolute())
    file_abs = str(file.absolute())
    profiles_abs = str(profiles.absolute())

    # Use a temp directory for dbt target path (needed for Bazel sandbox read-only filesystem)
    with tempfile.TemporaryDirectory() as tmp_target:
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
            file_abs,
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
                    dbt_target_path=tmp_target,
                    dbt_context=None,
                    dbt_project_dir=folder_abs,
                    dbt_profiles_dir=profiles_abs,
                )._asdict()
            ),
            {},
        )

    assert templated_file.sliced_file
    assert len(templated_file.sliced_file) > 1
    assert templated_file.raw_sliced
    assert len(templated_file.raw_sliced) > 1
