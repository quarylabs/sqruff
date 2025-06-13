#[cfg(all(
    not(target_os = "openbsd"),
    any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "powerpc64")
))]
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
