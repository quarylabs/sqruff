use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::utils::reflow::depth_map::DepthMap;
use sqruff_lib_core::parser::segments::Tables;
use std::hint::black_box;

include!("shims/global_alloc_overwrite.rs");

const COMPLEX_QUERY: &str = r#"-- Insert segments
INSERT INTO segments (id, name) VALUES
('uuid-1', 'Segment1'),
('uuid-2', 'Segment2'),
('uuid-3', 'Segment3');

-- Insert path steps
INSERT INTO path_steps (segment_id, idx, len, code_idxs) VALUES
('uuid-1', 1, 10, '{1,2,3}'),
('uuid-2', 2, 15, '{1,2}'),
('uuid-3', 3, 20, '{2,3,4}');

-- Function to calculate hash in PostgreSQL
CREATE OR REPLACE FUNCTION calculate_hash(val TEXT) RETURNS BIGINT AS $$
DECLARE
    result BIGINT;
BEGIN
    SELECT INTO result hashtext(val);
    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Function to construct DepthInfo from raw segment and stack
CREATE OR REPLACE FUNCTION construct_depth_info(segment_id UUID) RETURNS void AS $$
DECLARE
    raw RECORD;
    stack RECORD;
    stack_hashes BIGINT[];
    stack_positions JSONB;
BEGIN
    -- Select segment and path steps
    SELECT INTO raw id, name FROM segments WHERE id = segment_id;
    SELECT INTO stack array_agg(idx) AS idxs, array_agg(len) AS lens, array_agg(code_idxs) AS code_idxss
    FROM path_steps WHERE segment_id = raw.id;

    -- Calculate hashes
    SELECT INTO stack_hashes array_agg(calculate_hash(name::TEXT)) FROM segments WHERE id = segment_id;

    -- Construct positions
    SELECT INTO stack_positions jsonb_agg(jsonb_build_object(
        'idx', s.idx,
        'len', s.len,
        'type', CASE
            WHEN array_length(s.code_idxs, 1) = 0 THEN ''
            WHEN array_length(s.code_idxs, 1) = 1 THEN 'solo'
            WHEN s.idx = (SELECT min(c) FROM unnest(s.code_idxs) AS c) THEN 'start'
            WHEN s.idx = (SELECT max(c) FROM unnest(s.code_idxs) AS c) THEN 'end'
            ELSE ''
        END
    ))
    FROM path_steps s WHERE s.segment_id = raw.id;

    -- Insert into depth_info
    INSERT INTO depth_info (segment_id, stack_depth, stack_hashes, stack_positions)
    VALUES (segment_id, array_length(stack_hashes, 1), stack_hashes, stack_positions);
END;
$$ LANGUAGE plpgsql;

-- Example usage
SELECT construct_depth_info('uuid-1');
SELECT construct_depth_info('uuid-2');
SELECT construct_depth_info('uuid-3');"#;

fn depth_map(c: &mut Criterion) {
    let linter = Linter::new(FluffConfig::default(), None, None, false).unwrap();
    let tables = Tables::default();
    let tree = linter
        .parse_string(&tables, COMPLEX_QUERY, None)
        .unwrap()
        .tree
        .unwrap();

    c.bench_function("DepthMap::from_parent", |b| {
        b.iter(|| black_box(DepthMap::from_parent(&tree)));
    });
}

// Parse once so the measured growth is LT11 linting, not parser throughput.
fn lt11(c: &mut Criterion) {
    let config = FluffConfig::from_source("[sqruff]\ndialect = ansi\nrules = LT11\n", None);
    let linter = Linter::new(config, None, None, true).unwrap();
    let mut group = c.benchmark_group("LT11/union_all");
    for count in [100, 200, 400, 800] {
        let sql = (0..count)
            .map(|idx| format!("SELECT {idx} AS id"))
            .collect::<Vec<_>>()
            .join("\nUNION ALL\n");
        let tables = Tables::default();
        let parsed = linter.parse_string(&tables, &sql, None).unwrap();
        assert!(parsed.violations.is_empty());
        assert!(
            linter
                .lint_parsed(&tables, parsed.clone(), false)
                .unwrap()
                .violations()
                .is_empty()
        );
        group.bench_with_input(BenchmarkId::from_parameter(count), &parsed, |b, parsed| {
            b.iter(|| black_box(linter.lint_parsed(&tables, parsed.clone(), false).unwrap()));
        });
    }
    group.finish();
}

criterion_group!(benches, depth_map, lt11);
criterion_main!(benches);
