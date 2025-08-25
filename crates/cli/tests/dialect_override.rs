use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use expect_test::expect_file;

/// Tests that the dialect override works.
///
/// It tests the same file that is invalid ANSI but valid Postgres in the cases:
/// 1. with no config file to ensure it defaults to ANSI and fails
/// 2. with no config file but with a dialect override to ensure it succeeds
/// 3. with a config file set to ANSI to ensure it fails
/// 4. with a config file set to ANSI but with a dialect override to ensure it succeeds
fn main() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut lint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lint_dir.push("tests/lint");

    // Construct the path to the sqruff binary
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));


}

fn option_1() {
    // Create temp folder 
    

}



const statement: &str = "SELECT DISTINCT ON (customer_id)
       customer_id, total, created_at
FROM orders
ORDER BY customer_id, created_at DESC;";
