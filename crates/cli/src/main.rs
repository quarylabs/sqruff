#[cfg(all(
    target_os = "windows",
    not(feature = "dhat-heap"),
    feature = "mimalloc"
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(not(feature = "codegen-docs"))]
pub fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    std::process::exit(sqruff_cli_lib::run_with_args(std::env::args_os()));
}

#[cfg(feature = "codegen-docs")]
pub fn main() {
    sqruff_cli_lib::run_docs_generation();
}
