use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: bench <sqruff> <fixture> <iterations> [--allow-lint-errors]";
    let sqruff = PathBuf::from(args.next().ok_or(usage)?);
    let fixture = PathBuf::from(args.next().ok_or(usage)?);
    let iterations: usize = args.next().ok_or(usage)?.to_str().ok_or(usage)?.parse()?;
    let allow_lint_errors = match args.next() {
        None => false,
        Some(arg) if arg == "--allow-lint-errors" => true,
        Some(_) => return Err(usage.into()),
    };
    if iterations == 0 || args.next().is_some() {
        return Err(usage.into());
    }
    if !fixture.exists() {
        return Err(format!("Fixture does not exist: {}", fixture.display()).into());
    }

    for iteration in 1..=iterations {
        let start = Instant::now();
        let output = Command::new(&sqruff)
            .arg("lint")
            .arg(&fixture)
            .args(["-f", "human"])
            .output()?;
        let duration = start.elapsed();

        println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        println!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        println!("Iteration {iteration}/{iterations} took: {duration:?}");

        if !output.status.success() && !(allow_lint_errors && output.status.code() == Some(1)) {
            return Err(format!("sqruff failed: {}", output.status).into());
        }
    }
    Ok(())
}
