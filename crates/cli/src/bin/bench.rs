use std::process::Command;
use std::time::Instant;

fn main() {
    Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .unwrap();

    let start = Instant::now();

    let output = Command::new("target/release/sqruff")
        .args([
            "lint",
            "crates/lib-dialects/test/fixtures/dialects/ansi",
            "-f",
            "human",
        ])
        .output()
        .expect("Failed to execute process");

    let duration = start.elapsed();

    println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    println!("Execution took: {:?}", duration);
}
