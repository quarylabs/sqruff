use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;

const COMPLEX_QUERY: &str = r#"
WITH employee_data AS (
    SELECT emp.employee_id AS emp_id, emp.first_name, emp.last_name, emp.salary, emp.department_id, dept.department_name, emp.hire_date 
    FROM employees emp 
    JOIN departments dept ON emp.department_id = dept.department_id 
    WHERE emp.hire_date > DATE '2020-01-01'
),

department_salaries AS (
    SELECT department_id, AVG(salary) AS avg_salary -- Issue: Function name 'AVG' not immediately followed by parentheses
    FROM employees 
    GROUP BY department_id
),

recent_hires AS (
    SELECT e.employee_id, e.first_name, e.last_name, e.hire_date, e.salary, e.department_id 
    FROM employees e 
    WHERE e.hire_date > DATE '2021-01-01'
)

SELECT 
    e.emp_id, 
    e.first_name, 
    e.last_name, 
    e.salary, 
    e.department_name, 
    e.hire_date, 
    CASE 
        WHEN e.salary > ds.avg_salary THEN 'Above Average' 
        ELSE 'Below Average' 
    END AS salary_comparison, 
    rh.first_name AS rh_first_name, 
    rh.hire_date AS recent_hire_date, 
    COALESCE(e.salary, 0) AS adjusted_salary -- Issue: Function name 'COALESCE' not immediately followed by parentheses
FROM 
    employee_data e 
JOIN 
    department_salaries ds ON e.department_id = ds.department_id 
LEFT JOIN 
    recent_hires rh ON e.emp_id = rh.employee_id 
WHERE 
    e.department_id IN (
        SELECT department_id 
        FROM departments 
        WHERE location_id = 1700
    ) 
ORDER BY 
    e.last_name ASC, 
    e.first_name DESC, 
    e.salary;"#;

fn fix(c: &mut Criterion) {
    let passes = [("fix_complex_query", COMPLEX_QUERY)];

    let linter = Linter::new(FluffConfig::default(), None, None);

    for (name, source) in passes {
        let parsed = linter.parse_string(source.to_string(), None, None, None).unwrap();

        c.bench_function(name, |b| {
            b.iter(|| black_box(linter.lint_parsed(parsed.clone(), linter._rules.clone(), true)));
        });
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = fix
}
criterion_main!(benches);
