use super::linted_file::LintedFile;
use super::linter::Linter;

pub trait Runner: Sized {
    fn run(&mut self, paths: Vec<String>, fix: bool, linter: &mut Linter) -> Vec<LintedFile>;
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

impl<R: Runner> RunnerContext<'_, R> {
    pub fn run(&mut self, paths: Vec<String>, fix: bool) -> Vec<LintedFile> {
        self.runner.run(paths, fix, self.linter)
    }
}

pub struct SequentialRunner;

impl Runner for SequentialRunner {
    fn run(&mut self, paths: Vec<String>, fix: bool, linter: &mut Linter) -> Vec<LintedFile> {
        let mut acc = Vec::with_capacity(paths.len());
        let rule_pack = linter.get_rulepack();

        for path in paths {
            let rendered = linter.render_file(path);
            let linted_file = linter.lint_rendered(rendered, &rule_pack, fix);

            acc.push(linted_file);
        }

        acc
    }
}
