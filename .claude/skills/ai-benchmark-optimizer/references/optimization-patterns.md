# Common Rust Optimization Patterns

Quick reference for performance improvements commonly applicable to Rust codebases.

## Memory and Allocation

### Pattern 1: Reuse Allocations

**Before:**
```rust
fn process_items(items: &[Item]) -> Vec<Result> {
    items.iter().map(|item| {
        let mut buffer = Vec::new();  // Allocates each iteration
        process_with_buffer(item, &mut buffer)
    }).collect()
}
```

**After:**
```rust
fn process_items(items: &[Item]) -> Vec<Result> {
    let mut buffer = Vec::new();  // Single allocation, reused
    items.iter().map(|item| {
        buffer.clear();
        process_with_buffer(item, &mut buffer)
    }).collect()
}
```

### Pattern 2: Pre-allocate with Capacity

**Before:**
```rust
let mut results = Vec::new();
for item in items {
    results.push(process(item));
}
```

**After:**
```rust
let mut results = Vec::with_capacity(items.len());
for item in items {
    results.push(process(item));
}
```

### Pattern 3: Use SmallVec for Usually-Small Collections

```rust
use smallvec::SmallVec;

// Stack-allocated for up to 8 elements
fn process() -> SmallVec<[u8; 8]> {
    let mut result = SmallVec::new();
    // Usually stays on stack, heap only if >8 elements
    result
}
```

### Pattern 4: Avoid Intermediate Collections

**Before:**
```rust
let items: Vec<_> = input.iter().filter(|x| x.is_valid()).collect();
let results: Vec<_> = items.iter().map(|x| process(x)).collect();
```

**After:**
```rust
let results: Vec<_> = input
    .iter()
    .filter(|x| x.is_valid())
    .map(|x| process(x))
    .collect();  // Single collection
```

## String Operations

### Pattern 5: Use &str Instead of String When Possible

**Before:**
```rust
fn process(name: String) { /* ... */ }
process(input.to_string());
```

**After:**
```rust
fn process(name: &str) { /* ... */ }
process(input);  // No allocation
```

### Pattern 6: Use Cow for Conditional Ownership

```rust
use std::borrow::Cow;

fn process(input: &str) -> Cow<'_, str> {
    if needs_modification(input) {
        Cow::Owned(modify(input))  // Allocates only when needed
    } else {
        Cow::Borrowed(input)  // No allocation
    }
}
```

### Pattern 7: String Building with capacity

**Before:**
```rust
let mut result = String::new();
for part in parts {
    result.push_str(part);
}
```

**After:**
```rust
let total_len: usize = parts.iter().map(|s| s.len()).sum();
let mut result = String::with_capacity(total_len);
for part in parts {
    result.push_str(part);
}
```

## Data Structures

### Pattern 8: Use Faster Hash Maps

```rust
// Standard HashMap - cryptographically secure but slower
use std::collections::HashMap;

// FxHashMap - faster for non-security contexts
use rustc_hash::FxHashMap;

// AHashMap - good balance of speed and DOS resistance
use ahash::AHashMap;
```

### Pattern 9: Use IndexMap for Ordered Iteration

```rust
use indexmap::IndexMap;

// Maintains insertion order, faster than BTreeMap
let mut map = IndexMap::new();
```

### Pattern 10: Use HashSet for Membership Checks

**Before:**
```rust
fn contains_duplicate(items: &[Item]) -> bool {
    for i in 0..items.len() {
        for j in (i+1)..items.len() {
            if items[i] == items[j] {
                return true;
            }
        }
    }
    false
}
```

**After:**
```rust
fn contains_duplicate(items: &[Item]) -> bool {
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        if !seen.insert(item) {
            return true;
        }
    }
    false
}
```

## Control Flow

### Pattern 11: Early Returns

**Before:**
```rust
fn process(item: &Item) -> Option<Result> {
    if item.is_valid() {
        let processed = expensive_operation(item);
        if processed.is_ok() {
            Some(processed.unwrap())
        } else {
            None
        }
    } else {
        None
    }
}
```

**After:**
```rust
fn process(item: &Item) -> Option<Result> {
    if !item.is_valid() {
        return None;  // Early exit
    }
    expensive_operation(item).ok()
}
```

### Pattern 12: Avoid Redundant Checks

**Before:**
```rust
for item in items {
    if condition {
        process(item);
    }
}
```

**After (when condition is loop-invariant):**
```rust
if condition {
    for item in items {
        process(item);
    }
}
```

## Parallelism

### Pattern 13: Use Rayon for Data Parallelism

```rust
use rayon::prelude::*;

// Before: sequential
let results: Vec<_> = items.iter().map(expensive_fn).collect();

// After: parallel
let results: Vec<_> = items.par_iter().map(expensive_fn).collect();
```

### Pattern 14: Chunk Processing for Better Cache Locality

```rust
items.chunks(64).for_each(|chunk| {
    // Process chunk - better cache utilization
    for item in chunk {
        process(item);
    }
});
```

## Compiler Hints

### Pattern 15: Use #[inline] for Small Functions

```rust
#[inline]
fn small_hot_function(x: u32) -> u32 {
    x * 2 + 1
}

#[inline(always)]  // Force inlining
fn critical_path_function(x: u32) -> u32 {
    x * 2 + 1
}
```

### Pattern 16: Use cold Attribute for Error Paths

```rust
#[cold]
#[inline(never)]
fn handle_error(e: Error) {
    // Error handling code - rarely executed
}
```

### Pattern 17: Use likely/unlikely Hints

```rust
use std::intrinsics::{likely, unlikely};

if likely(common_case) {
    fast_path();
} else {
    slow_path();
}
```

## Unsafe Optimizations (Use Carefully)

### Pattern 18: Unchecked Array Access

```rust
// Safe version (bounds checked)
let value = array[index];

// Unsafe version (no bounds check)
let value = unsafe { *array.get_unchecked(index) };
// Only use when you can PROVE index is always valid
```

### Pattern 19: Skip UTF-8 Validation

```rust
// Safe version (validates UTF-8)
let s = String::from_utf8(bytes)?;

// Unsafe version (skips validation)
let s = unsafe { String::from_utf8_unchecked(bytes) };
// Only use when you KNOW bytes are valid UTF-8
```

## Measurement Tips

1. **Always benchmark in release mode**: `cargo bench` does this automatically
2. **Use black_box**: Prevent compiler from optimizing away results
3. **Warm up**: Let the CPU cache warm up before measuring
4. **Multiple runs**: Run benchmarks multiple times to reduce variance
5. **Isolated environment**: Close other applications, disable turbo boost for consistency
