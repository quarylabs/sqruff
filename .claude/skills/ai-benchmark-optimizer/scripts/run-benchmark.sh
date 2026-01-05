#!/bin/bash
# run-benchmark.sh - Run cargo benchmarks with reliability checks
#
# Usage: ./run-benchmark.sh <package> [benchmark-name] [--baseline <name>] [--save-baseline <name>]
#
# Examples:
#   ./run-benchmark.sh sqruff-lib
#   ./run-benchmark.sh sqruff-lib parse_simple_query
#   ./run-benchmark.sh sqruff-lib --save-baseline before
#   ./run-benchmark.sh sqruff-lib parse_simple_query --baseline before

set -e

PACKAGE=""
BENCHMARK=""
BASELINE=""
SAVE_BASELINE=""
RUNS=3
SAMPLE_SIZE=100

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --baseline)
            BASELINE="$2"
            shift 2
            ;;
        --save-baseline)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        --runs)
            RUNS="$2"
            shift 2
            ;;
        --sample-size)
            SAMPLE_SIZE="$2"
            shift 2
            ;;
        -*)
            echo "Unknown option: $1"
            exit 1
            ;;
        *)
            if [[ -z "$PACKAGE" ]]; then
                PACKAGE="$1"
            else
                BENCHMARK="$1"
            fi
            shift
            ;;
    esac
done

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: $0 <package> [benchmark-name] [options]"
    echo ""
    echo "Options:"
    echo "  --baseline <name>       Compare against saved baseline"
    echo "  --save-baseline <name>  Save results as baseline"
    echo "  --runs <n>              Number of runs for variance check (default: 3)"
    echo "  --sample-size <n>       Criterion sample size (default: 100)"
    exit 1
fi

# Build command
CMD="cargo bench -p $PACKAGE"

if [[ -n "$BENCHMARK" ]]; then
    CMD="$CMD -- $BENCHMARK"
else
    CMD="$CMD --"
fi

CMD="$CMD --sample-size $SAMPLE_SIZE"

if [[ -n "$BASELINE" ]]; then
    CMD="$CMD --baseline $BASELINE"
fi

if [[ -n "$SAVE_BASELINE" ]]; then
    CMD="$CMD --save-baseline $SAVE_BASELINE"
fi

echo "=== Benchmark Configuration ==="
echo "Package: $PACKAGE"
echo "Benchmark: ${BENCHMARK:-all}"
echo "Sample size: $SAMPLE_SIZE"
echo "Runs: $RUNS"
[[ -n "$BASELINE" ]] && echo "Comparing to baseline: $BASELINE"
[[ -n "$SAVE_BASELINE" ]] && echo "Saving baseline as: $SAVE_BASELINE"
echo ""

# Run benchmarks multiple times for variance check
echo "=== Running $RUNS iterations for variance check ==="
for i in $(seq 1 $RUNS); do
    echo ""
    echo "--- Run $i of $RUNS ---"
    eval "$CMD"
done

echo ""
echo "=== Benchmark Complete ==="
echo "Check above output for timing consistency across runs."
echo "Variance >5% indicates unreliable results."
