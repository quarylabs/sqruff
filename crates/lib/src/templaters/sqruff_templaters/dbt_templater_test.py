import json
import os
from pathlib import Path

from sqruff_templaters.dbt_templater import process_from_rust
from sqruff_templaters.python_templater import FluffConfig


def test_dbt():
    current = Path(os.path.dirname(os.path.abspath(__file__)))
    folder = current.joinpath("sample_dbt")
    file = folder.joinpath("models/example/my_first_dbt_model.sql")
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
