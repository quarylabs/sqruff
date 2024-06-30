use orx_concurrent_bag::ConcurrentBag;

use super::linted_file::LintedFile;
use super::linter::Linter;

pub trait Runner: Sized {
    fn run(
        &mut self,
        paths: Vec<String>,
        fix: bool,
        linter: &mut Linter,
    ) -> impl Iterator<Item = LintedFile>;
}

pub struct RunnerContext<'me, R> {
    linter: &'me mut Linter,
    runner: R,
}

impl<'me> RunnerContext<'me, ParallelRunner> {
    pub fn sequential(linter: &'me mut Linter) -> Self {
        Self { linter, runner: ParallelRunner }
    }
}

impl<R: Runner> RunnerContext<'_, R> {
    pub fn run(&mut self, paths: Vec<String>, fix: bool) -> impl Iterator<Item = LintedFile> + '_ {
        self.runner.run(paths, fix, self.linter)
    }
}

pub struct ParallelRunner;

impl Runner for ParallelRunner {
    fn run(
        &mut self,
        paths: Vec<String>,
        fix: bool,
        linter: &mut Linter,
    ) -> impl Iterator<Item = LintedFile> {
        use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};

        let rule_pack = linter.get_rulepack();

        let acc = ConcurrentBag::new();

        paths.par_iter().for_each(|path| {
            let rendered = linter.render_file(path.clone());
            let linted_file = linter.lint_rendered(rendered, &rule_pack, fix);
            acc.push(linted_file);
        });

        acc.into_inner().into_iter()
    }
}
