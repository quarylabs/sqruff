#[cfg(all(target_os = "windows", not(feature = "dhat-heap")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "dhat-heap")]
fn init_dhat() -> dhat::Profiler {
    dhat::Profiler::new_heap()
}

#[cfg(not(feature = "dhat-heap"))]
fn init_dhat() {}

#[cfg(not(feature = "codegen-docs"))]
pub fn main() {
    let _profiler = init_dhat();
    std::process::exit(sqruff_cli_lib::run_with_args(std::env::args_os()));
}

#[cfg(feature = "codegen-docs")]
pub fn main() {
    let _profiler = init_dhat();
    sqruff_cli_lib::run_docs_generation();
}
