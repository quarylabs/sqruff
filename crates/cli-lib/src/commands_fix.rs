use crate::commands::FixArgs;
use crate::commands::Format;
use crate::commands_lint::{ApplyFixes, Input, LintCommand, run_lint_command};
use sqruff_lib::api::{Mode, ParseErrors};
use sqruff_lib::core::config::FluffConfig;
use std::path::Path;

pub(crate) fn run_fix(
    args: FixArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    parse_errors: ParseErrors,
) -> i32 {
    let FixArgs { paths, format } = args;
    run_lint_command(
        LintCommand {
            mode: Mode::Fix,
            input: Input::Paths(paths),
            apply: ApplyFixes::ToDisk,
            format,
        },
        config,
        ignorer,
        parse_errors,
    )
}

pub(crate) fn run_fix_stdin(config: FluffConfig, format: Format, parse_errors: ParseErrors) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    run_lint_command(
        LintCommand {
            mode: Mode::Fix,
            input: Input::Stdin(read_in),
            apply: ApplyFixes::Stdout,
            format,
        },
        config,
        |_| false,
        parse_errors,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    fn ignore_none(_: &Path) -> bool {
        false
    }

    #[test]
    fn run_fix_does_not_update_mtime_when_no_changes() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "SELECT 1 FROM").unwrap();
        tmp.flush().unwrap();
        let tmp = tmp.into_temp_path();
        let path = tmp.to_path_buf();

        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        sleep(Duration::from_secs(1));

        let args = FixArgs {
            paths: vec![path.clone()],
            format: Format::Human,
        };
        let config = FluffConfig::default();
        run_fix(args, config, ignore_none, ParseErrors::Include);

        let after = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn run_fix_writes_file_when_changes_exist() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "SELECT foo bar FROM tabs").unwrap();
        tmp.flush().unwrap();
        let tmp = tmp.into_temp_path();
        let path = tmp.to_path_buf();

        let args = FixArgs {
            paths: vec![path.clone()],
            format: Format::Human,
        };
        let config = FluffConfig::from_source("[sqruff]\nrules = AL02\n", None);
        let exit_code = run_fix(args, config, ignore_none, ParseErrors::Include);

        assert_eq!(exit_code, 0);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "SELECT foo AS bar FROM tabs"
        );
    }
}
