---
name: ai-benchmark-optimizer
description: |
  Optimize Rust code performance using cargo benchmarks and AI-driven analysis.
  Use this skill when the user wants to: run cargo benchmarks, optimize performance,
  improve benchmark times, analyze profiling data, or iterate on performance improvements.
  Keywords: benchmark, optimize, performance, criterion, profiling, flamegraph, cargo bench.
---

# AI Benchmark Optimizer

A systematic approach to optimizing Rust code performance using cargo benchmarks as the feedback loop.

## Overview

This skill provides a structured methodology for:
1. Running reliable, reproducible benchmarks
2. Analyzing performance bottlenecks
3. Implementing targeted optimizations
4. Iterating until measurable improvements are achieved
5. Documenting the optimization journey

## Step 1: Benchmark Discovery and Setup

First, identify available benchmarks in the project:

```bash
# Find all benchmark files
find . -name "*.rs" -path "*/benches/*" 2>/dev/null

# Check Cargo.toml for benchmark definitions
grep -A 3 '\[\[bench\]\]' **/Cargo.toml
```

Common benchmark locations:
- `crates/*/benches/*.rs` - Criterion benchmarks
- `benches/*.rs` - Root-level benchmarks

## Step 2: Establish Baseline Measurements

### Running Benchmarks Reliably

For reliable benchmarks, minimize system noise:

```bash
# Run the full benchmark suite
cargo bench -p <package-name>

# Run a specific benchmark
cargo bench -p <package-name> -- <benchmark-name>

# Run with more iterations for stability
cargo bench -p <package-name> -- --sample-size 100

# Save baseline for comparison
cargo bench -p <package-name> -- --save-baseline before
```

### Benchmark Reliability Checklist

Before optimizing, ensure benchmarks are reliable:

1. **Consistency**: Run the same benchmark 3+ times, variance should be <5%
2. **Isolation**: Close other applications, disable background processes
3. **Warm-up**: Criterion handles this automatically, but verify
4. **Release mode**: Always benchmark with `--release` (cargo bench does this)
5. **Black box**: Ensure computed values aren't optimized away with `std::hint::black_box`

### Interpreting Criterion Output

```
benchmark_name    time:   [1.2345 µs 1.2456 µs 1.2567 µs]
                           ^^^^^^^^^ ^^^^^^^^^ ^^^^^^^^^
                           lower     estimate  upper
                           bound               bound
```

- Look for narrow confidence intervals (low variance)
- Track the **median** (middle value) for comparisons
- Changes of <3% are usually noise

## Step 3: Profiling and Analysis

### Generate Flamegraphs (Unix only)

If pprof is configured in the benchmark:

```bash
cargo bench -p <package-name> -- --profile-time 5
# Flamegraph SVG will be in target/criterion/<benchmark>/profile/flamegraph.svg
```

### Analyze with Samply (Alternative)

```bash
# Install samply
cargo install samply

# Profile a specific benchmark
cargo build --release -p <package-name>
samply record -- cargo bench -p <package-name> -- --profile-time 5
```

### Heap Profiling with DHAT

If the project supports dhat:

```bash
cargo bench -p <package-name> --features dhat-heap
```

### Identify Bottlenecks

When analyzing profiles, look for:
1. **Hot functions**: Functions consuming >10% of runtime
2. **Allocation patterns**: Excessive allocations in hot paths
3. **Cache misses**: Poor data locality
4. **Lock contention**: Thread synchronization overhead
5. **Redundant work**: Repeated computations that could be cached

## Step 4: Optimization Strategies

### Low-Hanging Fruit

1. **Avoid allocations in hot paths**
   - Reuse buffers with `Vec::clear()` instead of creating new ones
   - Use `String::with_capacity()` when size is known
   - Consider stack allocation for small, fixed-size data

2. **Reduce cloning**
   - Use references (`&T`) instead of owned values
   - Implement `Copy` for small types
   - Use `Cow<'a, T>` for sometimes-owned data

3. **Optimize data structures**
   - `SmallVec` for usually-small vectors
   - `AHashMap`/`FxHashMap` for faster hashing
   - Consider specialized structures (e.g., `IndexMap` for ordered iteration)

4. **Leverage iterators**
   - Avoid `collect()` when possible
   - Chain operations instead of intermediate collections
   - Use `par_iter()` from rayon for parallelism

### Algorithmic Improvements

1. **Caching/Memoization**: Cache expensive computations
2. **Early termination**: Return early when possible
3. **Batching**: Process multiple items together
4. **Lazy evaluation**: Delay computation until needed

### Rust-Specific Optimizations

1. **Use `#[inline]`** for small, frequently-called functions
2. **Enable LTO** in Cargo.toml: `lto = true`
3. **Use `codegen-units = 1`** for better optimization
4. **Profile-guided optimization (PGO)** for critical paths

## Step 5: Validate Improvements

After each optimization:

```bash
# Compare against baseline
cargo bench -p <package-name> -- --baseline before

# Run multiple times to confirm
for i in 1 2 3; do cargo bench -p <package-name> -- <benchmark-name>; done
```

### Interpret Changes

```
benchmark_name    time:   [1.0000 µs 1.0100 µs 1.0200 µs]
                  change: [-20.5% -19.2% -17.9%]
                           ^^^^^^ ^^^^^^ ^^^^^^
                           significant improvement!
```

- Changes of **>5%** are usually significant
- **Green (negative)** = faster = improvement
- **Red (positive)** = slower = regression

## Step 6: Iterate

Repeat the cycle:
1. Profile → Identify bottleneck
2. Hypothesize → Plan optimization
3. Implement → Make targeted change
4. Measure → Validate with benchmarks
5. Document → Record what worked/didn't

### Stop Conditions

Stop optimizing when:
- Returns diminish (<2% improvement per iteration)
- Code complexity increases significantly
- You've hit algorithmic limits
- Further gains require architectural changes

## Step 7: Generate Blog Post

After achieving meaningful improvements, generate documentation:

### Blog Post Structure

```markdown
# Optimizing [Component]: A [X]% Performance Improvement

## The Challenge
- What we were optimizing
- Initial performance baseline
- Why it mattered

## The Investigation
- Profiling methodology
- Key bottlenecks identified
- Analysis of hot paths

## The Journey
### Attempt 1: [Approach]
- What we tried
- Results (worked/didn't work)
- Lessons learned

### Attempt 2: [Approach]
...

## Final Results
- Before vs after comparison
- Percentage improvements
- Absolute time savings

## Key Takeaways
- What worked
- What didn't
- Recommendations for similar optimizations

## Code Examples
Show key code changes with before/after snippets.
```

## Example Workflow

```bash
# 1. Discover benchmarks
cargo bench -p sqruff-lib --benches -- --list

# 2. Run baseline
cargo bench -p sqruff-lib -- parse_simple_query --save-baseline before

# 3. Profile
cargo bench -p sqruff-lib -- parse_simple_query --profile-time 5

# 4. Make changes to hot path code
# ... edit source files ...

# 5. Measure improvement
cargo bench -p sqruff-lib -- parse_simple_query --baseline before

# 6. If improved, save new baseline
cargo bench -p sqruff-lib -- parse_simple_query --save-baseline after

# 7. Repeat for other benchmarks
```

## Tools Reference

| Tool | Purpose | Installation |
|------|---------|-------------|
| criterion | Statistical benchmarking | `cargo add criterion --dev` |
| pprof | CPU profiling/flamegraphs | Built into benchmarks |
| samply | Sampling profiler | `cargo install samply` |
| dhat | Heap profiling | Feature flag in Cargo.toml |
| perf | Linux performance counters | System package |
| hyperfine | Command-line benchmarking | `cargo install hyperfine` |

## Troubleshooting

### High Variance in Benchmarks

1. Close other applications
2. Disable CPU frequency scaling: `sudo cpupower frequency-set -g performance`
3. Increase sample size: `--sample-size 200`
4. Use `taskset` to pin to specific CPU cores

### Criterion Not Detecting Changes

1. Ensure changes are in release code path
2. Check that `black_box` is used properly
3. Verify benchmark is measuring the right thing
4. Look for caching effects between runs

### Flamegraph Empty or Unhelpful

1. Increase profile time: `--profile-time 10`
2. Ensure debug symbols are enabled in release
3. Check pprof feature is enabled
4. Verify the benchmark actually runs code (not just setup)

## Supporting Resources

This skill includes additional resources in its directory:

### Scripts (`scripts/`)

- **`run-benchmark.sh`**: Wrapper script for running benchmarks with reliability checks
  - Runs multiple iterations to check variance
  - Supports baseline comparison
  - Configurable sample size

- **`compare-baselines.sh`**: Compare two saved benchmark baselines

### References (`references/`)

- **`optimization-patterns.md`**: Quick reference for common Rust optimization patterns
  - Memory and allocation optimizations
  - String operation improvements
  - Data structure choices
  - Compiler hints and unsafe optimizations

- **`blog-post-template.md`**: Template for documenting optimization work
  - Structured format for the optimization journey
  - Before/after comparison sections
  - Lessons learned format

## AI-Assisted Optimization Loop

When using this skill with Claude, follow this iterative pattern:

1. **Discovery Phase**
   - Ask Claude to identify and list available benchmarks
   - Claude will run `cargo bench --list` and explore benchmark files

2. **Baseline Phase**
   - Ask Claude to establish baseline measurements
   - Claude will run benchmarks multiple times and report variance

3. **Analysis Phase**
   - Ask Claude to profile and identify bottlenecks
   - Claude will generate flamegraphs and analyze hot paths

4. **Optimization Phase**
   - Discuss potential optimizations with Claude
   - Claude will suggest specific code changes based on profiling data
   - Review and approve changes before implementation

5. **Validation Phase**
   - Ask Claude to re-run benchmarks
   - Claude will compare against baseline and report improvements

6. **Documentation Phase**
   - Ask Claude to generate a blog post
   - Claude will use the template and fill in with actual results

### Example Prompts

- "Run the parsing benchmark and establish a baseline"
- "Profile the fix_complex_query benchmark and identify bottlenecks"
- "What optimizations would help reduce allocations in the hot path?"
- "Implement the SmallVec optimization we discussed"
- "Compare current performance against the baseline"
- "Generate a blog post documenting our optimization journey"
