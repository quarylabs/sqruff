use super::core::Linter;
use super::linted_file::LintedFile;

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

        paths
            .par_iter()
            .map(|path| {
                let rendered = linter.render_file(path.clone());
                linter.lint_rendered(rendered, &rule_pack, fix)
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}
