use std::io::Read;
use std::path::PathBuf;

/// Check if the given input is the flag to use stdin as input.
///
/// If the input is a single path and that path is `-`, then the input is the flag to use stdin as
/// input. Else, the input is not the flag to use stdin as input.
///
/// The error message is returned if any of the inputs are `-` and there are other inputs.
pub(crate) fn is_std_in_flag_input(inputs: &[PathBuf]) -> Result<bool, String> {
    if inputs.len() == 1 && inputs[0] == PathBuf::from("-") {
        Ok(true)
    } else if inputs.iter().any(|input| *input == PathBuf::from("-")) {
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
