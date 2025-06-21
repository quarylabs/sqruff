#[cfg(target_os = "windows")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "codegen-docs"))]
pub fn main() {
    std::process::exit(sqruff_cli_lib::run_with_args(std::env::args_os()));
}

#[cfg(feature = "codegen-docs")]
pub fn main() {
    sqruff_cli_lib::run_docs_generation();
}
