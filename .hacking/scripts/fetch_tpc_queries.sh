#!/usr/bin/env bash
# Fetch the same pinned TPC-H / TPC-DS SQL corpus used by SQLFluff #7923.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
fixture_root="${1:-$repo_root/target/tpc-fixtures}"
doris_sha="3a2d9d55f1e8e2d74187179ef89c36c8562815fd"
raw_base="https://raw.githubusercontent.com/apache/doris/$doris_sha"

if [[ -f "$fixture_root/.doris-sha" ]] \
    && [[ "$(<"$fixture_root/.doris-sha")" == "$doris_sha" ]] \
    && [[ -f "$fixture_root/tpc-h/22.sql" ]] \
    && [[ -f "$fixture_root/tpc-ds/99.sql" ]]; then
    printf 'TPC fixtures already cached at %s\n' "$fixture_root"
    exit 0
fi

mkdir -p "$fixture_root/tpc-h" "$fixture_root/tpc-ds"

fetch_normalized() {
    curl --fail --location --silent --show-error --retry 3 "$1" \
        | sed -e 's/\r$//' -e 's/[[:blank:]]*$//' > "$2"
}

for n in {1..22}; do
    fetch_normalized "$raw_base/tools/tpch-tools/queries/q$n.sql" \
        "$fixture_root/tpc-h/$n.sql"
done

for n in {1..99}; do
    fetch_normalized "$raw_base/tools/tpcds-tools/queries/sf1/query$n.sql" \
        "$fixture_root/tpc-ds/$n.sql"
    case "$n" in
        14|23|24|39)
            printf '\n' >> "$fixture_root/tpc-ds/$n.sql"
            curl --fail --location --silent --show-error --retry 3 \
                "$raw_base/tools/tpcds-tools/queries/sf1/query${n}_1.sql" \
                | sed -e 's/\r$//' -e 's/[[:blank:]]*$//' \
                >> "$fixture_root/tpc-ds/$n.sql"
            ;;
    esac
done

# The marker is written last, so a failed or interrupted fetch is retried.
printf '%s\n' "$doris_sha" > "$fixture_root/.doris-sha"
printf 'Fetched TPC fixtures into %s\n' "$fixture_root"
