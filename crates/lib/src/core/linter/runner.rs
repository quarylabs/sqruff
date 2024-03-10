use super::linter::Linter;

pub trait Runner: Sized {
    fn run(&mut self, paths: Vec<String>, linter: &mut Linter);
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
    pub fn run(&mut self, paths: Vec<String>) {
        self.runner.run(paths, &mut self.linter);
    }
}

pub struct SequentialRunner;

impl Runner for SequentialRunner {
    fn run(&mut self, paths: Vec<String>, linter: &mut Linter) {
        for path in paths {
            let rendered = linter.render_file(path);
            linter.lint_rendered(rendered);
        }
    }
}
