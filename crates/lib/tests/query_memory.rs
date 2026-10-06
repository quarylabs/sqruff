use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;

struct CountingAllocator;

static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

// This integration test runs in its own process so other tests cannot affect
// the allocation count. Count live allocations, rather than allocator RSS,
// which can stay high even after memory has been freed.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(
                new_size as isize - layout.size() as isize,
                Ordering::Relaxed,
            );
        }
        ptr
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn repeated_st03_linting_releases_query_graphs() {
    let config = FluffConfig::from_source(
        "[sqruff]\ndialect = ansi\ntemplater = raw\nrules = ST03\n",
        None,
    );
    let mut linter = Linter::new(config, None, None, true).unwrap();
    let sql = "WITH first AS (SELECT 1 AS id), second AS (SELECT id FROM first), \
               third AS (SELECT id FROM second) SELECT id FROM third\n";

    // Warm up lazy rule and parser caches before measuring retained memory.
    for _ in 0..8 {
        let linted = linter.lint_string_wrapped(sql, false).unwrap();
        assert!(linted.violations().is_empty());
    }
    let before = LIVE_BYTES.load(Ordering::Relaxed);
    for _ in 0..128 {
        let linted = linter.lint_string_wrapped(sql, false).unwrap();
        assert!(linted.violations().is_empty());
    }
    let retained = LIVE_BYTES.load(Ordering::Relaxed) - before;

    // Allow small runtime/cache allocations, but not a retained graph per file.
    assert!(
        retained <= 16 * 1024,
        "ST03 retained {retained} bytes after linting 128 valid CTE queries"
    );
}
