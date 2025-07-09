pub mod core;
pub mod rules;
pub mod templaters;
#[cfg(test)]
mod tests;
pub mod utils;

pub trait Formatter: Send + Sync {
    fn dispatch_file_violations(&self, linted_file: &core::linter::linted_file::LintedFile);
    fn completion_message(&self, count: usize);
}
