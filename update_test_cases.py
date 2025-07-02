# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "ruamel.yaml==0.18.14",
# ]
# ///

from ruamel.yaml import YAML
from pathlib import Path
import sys

if len(sys.argv) != 2:
    raise Exception("Usage: uv run update_test_cases.py <path_to_sqlfluff>")

sqlfluff_path = sys.argv[1]

sqlfluff_test_cases_path = Path(f"{sqlfluff_path}/test/fixtures/rules/std_rule_cases")

if not sqlfluff_test_cases_path.exists():
    raise Exception(f"Path {sqlfluff_test_cases_path} does not exist.")

sqruff_test_cases_path = Path("./crates/lib/test/fixtures/rules/std_rule_cases")

IGNORED_FILES = [
    "LT01-functions.yml",
    "TQ01.yml",
    "JJ01.yml",
    "AM08.yml",
    "ST10.yml",
    "LT15.yml",
    "LT14.yml",
    "ST11.yml",
    "CV12.yml",
    "AL09.yml", # TODO add this file because AL09 is implemented
    "CP02_LT01.yml", # TODO add this file 
]

yaml = YAML()

for file in sqlfluff_test_cases_path.glob("*.yml"):
    if file.name in IGNORED_FILES:
        print(f"Skipping {file.name}...")
        continue

    sqlfluff_file_content = file.read_text()
    sqlfluff_file_content_lines = sqlfluff_file_content.splitlines()
    sqlfluff_yaml = yaml.load(sqlfluff_file_content)

    sqruff_test_file = Path(sqruff_test_cases_path, file.name)

    print(f"Checking {sqruff_test_file}...")

    if not sqruff_test_file.exists():
        raise Exception(f"File {sqruff_test_file} does not exist.")

    sqruff_yaml = yaml.load(sqruff_test_file)

    ignored_cases = {}

    for test_case_name in sqruff_yaml:
        test_case = sqruff_yaml[test_case_name]

        if "ignored" in test_case:
            ignored_cases[test_case_name] = test_case["ignored"]

    for test_case_name in sqlfluff_yaml:
        test_case = sqlfluff_yaml[test_case_name]

        if test_case_name in ignored_cases:
            test_case_line_index = sqlfluff_file_content_lines.index(
                f"{test_case_name}:"
            )

            # some files use 2 spaces, some files use 4 spaces
            first_value_in_test_case = sqlfluff_file_content_lines[
                test_case_line_index + 1
            ]
            indent = len(first_value_in_test_case) - len(
                first_value_in_test_case.lstrip(" ")
            )

            sqlfluff_file_content_lines.insert(
                test_case_line_index + 1,
                f'{" " * indent}ignored: "{ignored_cases[test_case_name]}"',
            )

    with open(sqruff_test_file, "w") as f:
        f.write("\n".join(sqlfluff_file_content_lines) + "\n")
