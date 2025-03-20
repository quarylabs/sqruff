use criterion::{Criterion, black_box, criterion_group, criterion_main};
#[cfg(unix)]
use pprof::criterion::{Output, PProfProfiler};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::test_functions::fresh_ansi_dialect;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::context::ParseContext;
use sqruff_lib_core::parser::matchable::MatchableTrait as _;
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_core::parser::segments::test_functions::lex;

include!("shims/global_alloc_overwrite.rs");

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
        let config_for_parser = config.clone();
        let parser: Parser = (&config_for_parser).into();
        let mut ctx: ParseContext = (&parser).into();
        let segment = dialect.r#ref("FileSegment");
        let mut segments = lex(config.get_dialect(), source);

        if segments.last().unwrap().get_type() == SyntaxKind::EndOfFile {
            segments.pop();
        }

        c.bench_function(name, |b| {
            b.iter(|| {
                let match_result = segment.match_segments(&segments, 0, &mut ctx).unwrap();
                black_box(match_result);
            });
        });
    }
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
