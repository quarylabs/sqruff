#!/usr/bin/env python3
"""Benchmark for dbt templater optimizations.

This benchmark compares three approaches:
1. Old approach: Create a new DbtTemplater instance for each file
2. Cached approach: Reuse a cached DbtTemplater instance per project
3. Batch approach: Process all files in a single batch with shared state

The key optimization is that expensive operations like manifest loading,
config parsing, and compiler initialization only happen once when using
caching or batch processing.

Usage:
    python -m sqruff.templaters.dbt_templater_benchmark

Requirements:
    - dbt-core and dbt-duckdb (or another adapter) must be installed
    - Run from the sqruff project root or set DBT_PROJECT_DIR appropriately
"""

import json
import os
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Tuple

# Add the parent directory to the path for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from sqruff.templaters.dbt_templater import (
    DbtTemplater,
    clear_templater_cache,
    process_batch_from_rust,
    process_from_rust,
)
from sqruff.templaters.python_templater import FluffConfig


def get_test_project_path() -> Path:
    """Get the path to the test dbt project."""
    # Try to find the test project relative to this file
    this_file = Path(__file__).resolve()

    # Navigate up to find the dbt_sample directory
    potential_paths = [
        this_file.parent.parent.parent.parent / "tests" / "dbt_sample",
        Path.cwd() / "crates" / "cli-python" / "tests" / "dbt_sample",
        Path(os.environ.get("DBT_PROJECT_DIR", ""))
        if os.environ.get("DBT_PROJECT_DIR")
        else None,
    ]

    for path in potential_paths:
        if path and path.exists() and (path / "dbt_project.yml").exists():
            return path

    raise FileNotFoundError(
        "Could not find dbt_sample test project. "
        "Please run from the sqruff project root or set DBT_PROJECT_DIR."
    )


def get_test_files(project_path: Path) -> List[Tuple[str, str]]:
    """Get all SQL model files from the test project."""
    models_path = project_path / "models"
    files = []

    for sql_file in models_path.rglob("*.sql"):
        content = sql_file.read_text()
        files.append((content, str(sql_file.resolve())))

    return files


def create_config(project_path: Path) -> Tuple[FluffConfig, str, Dict[str, Any]]:
    """Create a FluffConfig for the test project."""
    profiles_path = project_path / "profiles"

    config = FluffConfig(
        dialect="duckdb",
        templater="dbt",
        dbt_project_dir=str(project_path),
        dbt_profiles_dir=str(profiles_path),
    )

    config_dict = {
        "dialect": "duckdb",
        "templater": "dbt",
        "dbt_project_dir": str(project_path),
        "dbt_profiles_dir": str(profiles_path),
    }
    config_string = json.dumps(config_dict)
    live_context: Dict[str, Any] = {}

    return config, config_string, live_context


def benchmark_old_approach(
    files: List[Tuple[str, str]],
    config: FluffConfig,
    live_context: Dict[str, Any],
    iterations: int = 3,
) -> float:
    """Benchmark the old approach: new templater per file.

    This simulates the previous behavior where a new DbtTemplater
    was created for each file, causing manifest loading to happen
    repeatedly.
    """
    times = []

    for i in range(iterations):
        # Clear the cache to simulate the old behavior
        clear_templater_cache()

        start = time.perf_counter()

        for content, fname in files:
            # Create a NEW templater for each file (old behavior)
            templater = DbtTemplater(
                override_context=live_context, sqlfluff_config=config
            )
            try:
                fnames = templater.sequence_files([fname], config=config)
                fname = fnames[0]
                output, errors = templater.process(
                    in_str=content,
                    fname=fname,
                    context=live_context,
                    config=config,
                )
            except Exception as e:
                print(f"  Error processing {fname}: {e}")

        elapsed = time.perf_counter() - start
        times.append(elapsed)
        print(f"  Iteration {i + 1}: {elapsed:.3f}s")

    return sum(times) / len(times)


def benchmark_cached_approach(
    files: List[Tuple[str, str]],
    config: FluffConfig,
    config_string: str,
    live_context: Dict[str, Any],
    iterations: int = 3,
) -> float:
    """Benchmark the cached approach: reuse templater instance.

    This uses the new _get_or_create_templater function which
    caches the templater instance per project directory.
    """
    times = []

    for i in range(iterations):
        # Clear the cache to ensure fair comparison
        clear_templater_cache()

        start = time.perf_counter()

        for content, fname in files:
            # This will reuse the cached templater after the first file
            try:
                process_from_rust(
                    string=content,
                    fname=fname,
                    config_string=config_string,
                    live_context=live_context,
                )
            except Exception as e:
                print(f"  Error processing {fname}: {e}")

        elapsed = time.perf_counter() - start
        times.append(elapsed)
        print(f"  Iteration {i + 1}: {elapsed:.3f}s")

    return sum(times) / len(times)


def benchmark_batch_approach(
    files: List[Tuple[str, str]],
    config_string: str,
    live_context: Dict[str, Any],
    iterations: int = 3,
) -> float:
    """Benchmark the batch approach: process all files together.

    This uses the new process_batch_from_rust function which
    sequences all files together and processes them with a single
    templater instance.
    """
    times = []

    for i in range(iterations):
        # Clear the cache to ensure fair comparison
        clear_templater_cache()

        start = time.perf_counter()

        try:
            results = process_batch_from_rust(
                files=files,
                config_string=config_string,
                live_context=live_context,
            )
            # Check for errors
            for templated_file, error in results:
                if error:
                    print(f"  Error: {error}")
        except Exception as e:
            print(f"  Error in batch processing: {e}")

        elapsed = time.perf_counter() - start
        times.append(elapsed)
        print(f"  Iteration {i + 1}: {elapsed:.3f}s")

    return sum(times) / len(times)


def run_benchmark():
    """Run the complete benchmark suite."""
    print("=" * 60)
    print("DBT Templater Benchmark")
    print("=" * 60)

    # Setup
    print("\nSetup:")
    try:
        project_path = get_test_project_path()
        print(f"  Project path: {project_path}")
    except FileNotFoundError as e:
        print(f"  Error: {e}")
        return

    files = get_test_files(project_path)
    print(f"  Found {len(files)} SQL files")

    if not files:
        print("  No SQL files found, exiting.")
        return

    for content, fname in files:
        print(f"    - {Path(fname).name} ({len(content)} chars)")

    try:
        config, config_string, live_context = create_config(project_path)
    except Exception as e:
        print(f"  Error creating config: {e}")
        return

    iterations = 3
    print(f"\n  Running {iterations} iterations for each approach...")

    # Benchmark old approach
    print("\n" + "-" * 60)
    print("Benchmark 1: Old Approach (new templater per file)")
    print("-" * 60)
    try:
        old_time = benchmark_old_approach(files, config, live_context, iterations)
        print(f"  Average: {old_time:.3f}s")
    except Exception as e:
        print(f"  Failed: {e}")
        old_time = None

    # Benchmark cached approach
    print("\n" + "-" * 60)
    print("Benchmark 2: Cached Approach (reuse templater)")
    print("-" * 60)
    try:
        cached_time = benchmark_cached_approach(
            files, config, config_string, live_context, iterations
        )
        print(f"  Average: {cached_time:.3f}s")
    except Exception as e:
        print(f"  Failed: {e}")
        cached_time = None

    # Benchmark batch approach
    print("\n" + "-" * 60)
    print("Benchmark 3: Batch Approach (process all together)")
    print("-" * 60)
    try:
        batch_time = benchmark_batch_approach(
            files, config_string, live_context, iterations
        )
        print(f"  Average: {batch_time:.3f}s")
    except Exception as e:
        print(f"  Failed: {e}")
        batch_time = None

    # Summary
    print("\n" + "=" * 60)
    print("Summary")
    print("=" * 60)
    print(f"  Files processed: {len(files)}")
    print(f"  Iterations: {iterations}")
    print()

    if old_time is not None:
        print(f"  Old approach (new templater/file): {old_time:.3f}s")
    if cached_time is not None:
        print(f"  Cached approach (reuse templater): {cached_time:.3f}s")
    if batch_time is not None:
        print(f"  Batch approach (all together):     {batch_time:.3f}s")

    print()

    if old_time and cached_time:
        speedup = old_time / cached_time
        print(f"  Cached vs Old speedup: {speedup:.2f}x")

    if old_time and batch_time:
        speedup = old_time / batch_time
        print(f"  Batch vs Old speedup:  {speedup:.2f}x")

    if cached_time and batch_time:
        speedup = cached_time / batch_time
        print(f"  Batch vs Cached speedup: {speedup:.2f}x")

    print()
    print("Note: The speedup increases with more files, as manifest loading")
    print("      only happens once instead of once per file.")


if __name__ == "__main__":
    run_benchmark()
