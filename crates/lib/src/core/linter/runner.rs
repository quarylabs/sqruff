use rayon::prelude::*;

use super::linted_file::LintedFile;
use super::linter::Linter;

pub trait Runner: Sized {
    fn run<'a>(
        &'a mut self,
        paths: &'a [String],
        fix: bool,
        linter: &'a mut Linter,
    ) -> Box<dyn Iterator<Item = LintedFile> + 'a>;
}

pub struct RunnerContext<'me, R> {
    linter: &'me mut Linter,
    runner: R,
}

impl<'me> RunnerContext<'me, SequentialRunner> {
    pub fn sequential(linter: &'me mut Linter) -> Self {
        Self { linter, runner: SequentialRunner }
    }
}

impl<'me> RunnerContext<'me, ParallelRunner> {
    pub fn parallel(linter: &'me mut Linter) -> Self {
        Self { linter, runner: ParallelRunner }
    }
}

impl<R: Runner> RunnerContext<'_, R> {
    pub fn run<'a>(
        &'a mut self,
        paths: &'a [String],
        fix: bool,
    ) -> Box<dyn Iterator<Item = LintedFile> + 'a> {
        self.runner.run(paths, fix, &mut self.linter)
    }
}

pub struct SequentialRunner;

impl Runner for SequentialRunner {
    fn run<'a>(
        &'a mut self,
        paths: &'a [String],
        fix: bool,
        linter: &'a mut Linter,
    ) -> Box<dyn Iterator<Item = LintedFile> + 'a> {
        Box::new(paths.iter().map(move |path| {
            let rendered = linter.render_file(path.to_string());
            let rule_pack = linter.get_rulepack();
            linter.lint_rendered(rendered, &rule_pack, fix)
        }))
    }
}

pub struct ParallelRunner;

impl Runner for ParallelRunner {
    fn run<'a>(
        &'a mut self,
        paths: &'a [String],
        fix: bool,
        linter: &'a mut Linter,
    ) -> Box<dyn Iterator<Item = LintedFile> + 'a> {
        let rule_pack = linter.get_rulepack();
        Box::new(paths.iter().map(move |path| {
            let rendered = linter.render_file(path.clone());
            linter.lint_rendered(rendered, &rule_pack, fix)
        }))
    }
}
