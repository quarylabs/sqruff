use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sqlflufff::core::parser::segments::fix::FixPatch;
use sqlflufff::core::linter::linted_file::LintedFile;

pub fn criterion_benchmark(c: &mut Criterion) {
    let tests = [
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

    c.bench_function("linted file", |b| b.iter(|| {
        for (source_file_slices, source_patches, raw_source_string, expected_result) in &tests {
            let result = LintedFile::build_up_fixed_source_string(
                &source_file_slices,
                &source_patches,
                raw_source_string,
            );
        }
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);