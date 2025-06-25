use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
#[cfg(unix)]
use pprof::criterion::{Output, PProfProfiler};

include!("shims/global_alloc_overwrite.rs");

fn bench_placeholder_templater(c: &mut Criterion) {
    use sqruff_lib::core::config::FluffConfig;
    use sqruff_lib::templaters::Templater;
    use sqruff_lib::templaters::placeholder::PlaceholderTemplater;
    
    let config = FluffConfig::from_source(
        r#"
[sqruff:templater:placeholder]
param_style = dollar
id = 123
name = test_name
value = 456
user_status = active
start_date = 2024-01-01
end_date = 2024-12-31
"#,
        None,
    );
    
    let templater = PlaceholderTemplater::default();
    
    let small_sql = r#"
SELECT $id, $name FROM users WHERE id = $id
"#;
    
    let medium_sql = r#"
-- This is a comment with $id placeholder
SELECT 
    $id,  -- Another $id in comment
    $name,
    /* Block comment with $id 
       and $name placeholders */
    $value
FROM users
WHERE user_id = $id -- Final $id in comment
  AND name = $name
  AND status = $user_status
"#;
    
    let large_sql = generate_large_sql_with_comments(1000); // 1000 placeholders
    let very_large_sql = generate_large_sql_with_comments(10000); // 10000 placeholders
    
    let mut group = c.benchmark_group("placeholder_templater");
    
    // Set sample size for more stable results
    group.sample_size(200);
    
    // Benchmark different SQL sizes
    group.bench_function("small_sql", |b| {
        b.iter(|| {
            let result = templater.process(
                black_box(small_sql), 
                "test.sql", 
                &config, 
                &None
            ).unwrap();
            black_box(result);
        })
    });
    
    group.bench_function("medium_sql_with_comments", |b| {
        b.iter(|| {
            let result = templater.process(
                black_box(medium_sql), 
                "test.sql", 
                &config, 
                &None
            ).unwrap();
            black_box(result);
        })
    });
    
    group.throughput(Throughput::Bytes(large_sql.len() as u64));
    group.bench_function("large_sql_1k_placeholders", |b| {
        b.iter(|| {
            let result = templater.process(
                black_box(&large_sql), 
                "test.sql", 
                &config, 
                &None
            ).unwrap();
            black_box(result);
        })
    });
    
    group.throughput(Throughput::Bytes(very_large_sql.len() as u64));
    group.bench_function("very_large_sql_10k_placeholders", |b| {
        b.iter(|| {
            let result = templater.process(
                black_box(&very_large_sql), 
                "test.sql", 
                &config, 
                &None
            ).unwrap();
            black_box(result);
        })
    });
    
    group.finish();
}

fn generate_large_sql_with_comments(num_placeholders: usize) -> String {
    let mut sql = String::with_capacity(num_placeholders * 100);
    
    sql.push_str("-- Large SQL query with many placeholders\n");
    sql.push_str("SELECT\n");
    
    for i in 0..num_placeholders {
        if i > 0 {
            sql.push_str(",\n");
        }
        
        // Mix of regular placeholders and comments
        if i % 10 == 0 {
            sql.push_str(&format!("    $param{} -- Comment with $param{} placeholder", i, i));
        } else if i % 15 == 0 {
            sql.push_str(&format!("    /* Block comment $param{} */ $param{}", i, i));
        } else {
            sql.push_str(&format!("    $param{}", i));
        }
    }
    
    sql.push_str("\nFROM large_table\nWHERE ");
    
    // Add some WHERE conditions
    for i in 0..10 {
        if i > 0 {
            sql.push_str("\n  AND ");
        }
        sql.push_str(&format!("col{} = $value{}", i, i));
    }
    
    sql
}

#[cfg(unix)]
criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_placeholder_templater
}

#[cfg(not(unix))]
criterion_group!(benches, bench_placeholder_templater);

criterion_main!(benches);