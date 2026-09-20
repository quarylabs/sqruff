use std::io::Read;
use std::path::{Path, PathBuf};

use sqruff_lib::core::linter::discovery::paths_from_path_check_non_existent;

/// Check if the given input is the flag to use stdin as input.
///
/// If the input is a single path and that path is `-`, then the input is the flag to use stdin as
/// input. Else, the input is not the flag to use stdin as input.
///
/// The error message is returned if any of the inputs are `-` and there are other inputs.
pub(crate) fn is_std_in_flag_input(inputs: &[PathBuf]) -> Result<bool, String> {
    if inputs.len() == 1 && &inputs[0] == "-" {
        Ok(true)
    } else if inputs.iter().any(|input| input == "-") {
        Err("Cannot mix stdin flag with other inputs".to_string())
    } else {
        Ok(false)
    }
}

/// Read the contents of stdin.
pub(crate) fn read_std_in() -> Result<String, String> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}

/// Return whether a virtual stdin filename is excluded by ignore files.
pub(crate) fn stdin_filename_is_ignored(
    filename: Option<&Path>,
    ignore_files: bool,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> bool {
    let Some(filename) = filename else {
        return false;
    };

    let paths = paths_from_path_check_non_existent(
        filename.to_path_buf(),
        None,
        None,
        Some(ignore_files),
        None,
        &[String::new()],
        Some(ignorer),
    );
    let ignored = paths.is_empty();
    if ignored {
        eprintln!(
            "Exact file path {} was given but it was ignored by an ignore pattern; re-run with `--disregard-sqlfluffignores` to not process ignore files.",
            filename.display()
        );
    }
    ignored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_std_in_flag_input() {
        let inputs = vec![PathBuf::from("-")];
        assert_eq!(is_std_in_flag_input(&inputs), Ok(true));

        let inputs = vec![PathBuf::from("file1"), PathBuf::from("-")];
        assert_eq!(
            is_std_in_flag_input(&inputs),
            Err("Cannot mix stdin flag with other inputs".to_string())
        );

        let inputs = vec![PathBuf::from("file1")];
        assert_eq!(is_std_in_flag_input(&inputs), Ok(false));
    }
}
