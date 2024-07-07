use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(unix)]
use pprof::criterion::{Output, PProfProfiler};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::dialects::base::Dialect;
use sqruff_lib::core::parser::context::ParseContext;
use sqruff_lib::core::parser::matchable::Matchable;
use sqruff_lib::core::parser::segments::base::ErasedSegment;
use sqruff_lib::core::parser::segments::test_functions::{fresh_ansi_dialect, lex};

const SIMPLE_QUERY: &str = r#"select 1 from dual"#;

const EXPRESSION_RECURSION: &str = r#"select
1
from
test_table
where
test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' --5
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' -- 10
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' -- 15
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' -- 20
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' --30
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%'
or test_table.string_field like 'some string%' -- 40"#;

const COMPLEX_QUERY: &str = r#"select
t1.id,
t2.name,
case
    when t1.value > 100 then 'High'
    else 'Low'
end as value_category,
count(*) over (partition by t1.category) as category_count
from
table1 t1
join table2 t2 on t1.id = t2.id
where
t1.date > '2023-01-01'
and (
    t2.status = 'active'
    or t2.status = 'pending'
)
order by t1.id desc"#;

fn parse(c: &mut Criterion) {
    let dialect = fresh_ansi_dialect();

    let passes = [
        ("parse_simple_query", SIMPLE_QUERY),
        ("parse_expression_recursion", EXPRESSION_RECURSION),
        ("parse_complex_query", COMPLEX_QUERY),
    ];

    for (name, source) in passes {
        let config = FluffConfig::default();
        let (mut ctx, segment, segments) = mk_segments(&dialect, &config, source);
        c.bench_function(name, |b| {
            b.iter(|| {
                let match_result = segment.match_segments(&segments, 0, &mut ctx).unwrap();
                black_box(match_result);
            });
        });
    }
}

fn mk_segments<'a>(
    dialect: &'a Dialect,
    config: &FluffConfig,
    source: &str,
) -> (ParseContext<'a>, std::sync::Arc<dyn Matchable>, Vec<ErasedSegment>) {
    let ctx = ParseContext::new(dialect, <_>::default());
    let segment = dialect.r#ref("FileSegment");
    let mut segments = lex(config, source);

    if segments.last().unwrap().get_type() == "end_of_file" {
        segments.pop();
    }

    (ctx, segment, segments)
}

#[cfg(unix)]
criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = parse
}

#[cfg(not(unix))]
criterion_group!(benches, parse);

criterion_main!(benches);
