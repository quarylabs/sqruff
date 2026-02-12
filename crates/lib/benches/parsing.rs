use criterion::{Criterion, criterion_group, criterion_main};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;
use std::hint::black_box;

include!("shims/global_alloc_overwrite.rs");

const SIMPLE_QUERY: &str = r#"select 1 from dual"#;
const SUPERLONG_QUERY: &str = include_str!("superlong.sql");

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
    let passes = [
        ("parse_simple_query", SIMPLE_QUERY),
        ("parse_expression_recursion", EXPRESSION_RECURSION),
        ("parse_complex_query", COMPLEX_QUERY),
        ("parse_superlong", SUPERLONG_QUERY),
    ];

    for (name, source) in passes {
        let config = FluffConfig::default();
        let parser: Parser = (&config).into();
        let tables = Tables::default();
        let lexer = Lexer::from(config.get_dialect());
        let (segments, errors) = lexer.lex(&tables, source);
        assert!(errors.is_empty());

        c.bench_function(name, |b| {
            b.iter(|| {
                let parsed = parser.parse(&tables, &segments).unwrap();
                black_box(parsed);
            });
        });
    }
}

criterion_group!(benches, parse);
criterion_main!(benches);
