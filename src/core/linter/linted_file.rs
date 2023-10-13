use crate::core::parser::segments::fix::FixPatch;
use std::ops::Range;

pub struct LintedFile;

impl LintedFile {
    pub fn build_up_fixed_source_string(
        source_file_slices: Vec<Range<usize>>,
        source_patches: Vec<FixPatch>,
        raw_source_string: &str,
    ) -> String {
        // Use patches and raw file to fix the source file.
        //
        // This assumes that patches and slices have already
        // been coordinated. If they haven't then this will
        // fail because we rely on patches having a corresponding
        // slice of exactly the right file in the list of file
        // slices.

        // Iterate through the patches, building up the new string.
        let mut str_buff = String::new();
        for source_slice in source_file_slices.iter() {
            // Is it one in the patch buffer:
            let mut is_patched = false;
            for patch in source_patches.iter() {
                if patch.source_slice == *source_slice {
                    // Use the patched version
                    // Note: Logging is omitted here, but you can use the `log` crate
                    str_buff.push_str(&patch.fixed_raw);
                    is_patched = true;
                    break;
                }
            }
            if !is_patched {
                // Use the raw string
                str_buff.push_str(&raw_source_string[source_slice.start..source_slice.end]);
            }
        }
        str_buff
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test _build_up_fixed_source_string. This is part of fix_string().
    #[test]
    fn test__linted_file__build_up_fixed_source_string() {
        let tests = vec![
            // Trivial example
            (vec![0..1], vec![], "a", "a"),
            // Simple replacement
            (
                vec![(0..1), (1..2), (2..3)],
                vec![FixPatch::new(
                    1..2,
                    "d".to_string(),
                    "".to_string(),
                    1..2,
                    "b".to_string(),
                    "b".to_string(),
                )],
                "abc",
                "adc",
            ),
            // Simple insertion
            (
                vec![(0..1), (1..1), (1..2)],
                vec![FixPatch::new(
                    1..1,
                    "b".to_string(),
                    "".to_string(),
                    1..1,
                    "".to_string(),
                    "".to_string(),
                )],
                "ac",
                "abc",
            ),
            // Simple deletion
            (
                vec![(0..1), (1..2), (2..3)],
                vec![FixPatch::new(
                    1..2,
                    "".to_string(),
                    "".to_string(),
                    1..2,
                    "b".to_string(),
                    "b".to_string(),
                )],
                "abc",
                "ac",
            ),
            // Illustrative templated example (although practically at this step, the routine shouldn't care if it's templated).
            (
                vec![(0..2), (2..7), (7..9)],
                vec![FixPatch::new(
                    2..3,
                    "{{ b }}".to_string(),
                    "".to_string(),
                    2..7,
                    "b".to_string(),
                    "{{b}}".to_string(),
                )],
                "a {{b}} c",
                "a {{ b }} c",
            ),
        ];

        for test in tests {
            let result =
                LintedFile::build_up_fixed_source_string(test.0.to_vec(), test.1.to_vec(), test.2);

            assert_eq!(result, test.3)
        }
    }
}
