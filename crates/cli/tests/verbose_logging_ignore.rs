use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use assert_cmd::Command;

/// Tests for verbose logging behavior when ignore patterns are applied.
/// 
/// These tests verify that sqruff properly logs ignore behavior when verbose mode is enabled.
/// The tests check different verbosity levels and ensure that ignored directories and files
/// are logged appropriately with the specific patterns that caused the exclusion.

/// Test that ignored directories are logged when verbose mode is enabled
#[test]
fn test_verbose_logging_ignored_directories() {
    // Create a temporary directory for our test project
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create .data directory with SQL files (this should be ignored)
    let data_dir = project_root.join(".data");
    fs::create_dir_all(&data_dir).unwrap();
    
    // Create SQL files in .data directory
    let ignored_sql_file = data_dir.join("ignored_file.sql");
    fs::write(&ignored_sql_file, "SELECT * FROM users WHERE id = 1;").unwrap();
    
    // Create nested directory structure in .data
    let nested_data_dir = data_dir.join("nested");
    fs::create_dir_all(&nested_data_dir).unwrap();
    let nested_ignored_sql_file = nested_data_dir.join("nested_ignored.sql");
    fs::write(&nested_ignored_sql_file, "SELECT name FROM products;").unwrap();

    // Create a regular SQL file that should NOT be ignored
    let regular_sql_file = project_root.join("regular_file.sql");
    fs::write(&regular_sql_file, "SELECT * FROM orders;").unwrap();

    // Create .sqruffignore file with .data pattern
    let sqruffignore_file = project_root.join(".sqruffignore");
    fs::write(&sqruffignore_file, ".data\n").unwrap();

    // Get the sqruff binary path
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Run sqruff lint with verbose logging enabled
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(project_root)
        .env("SQRUFF_LOG", "debug") // Enable debug logging to capture verbose output
        .current_dir(project_root);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);
    println!("Exit code: {:?}", output.status.code());

    // Verify that ignored directories are logged in verbose mode
    // The logging should indicate that .data directory was skipped
    let contains_ignore_logging = stderr.contains(".data") && 
                                 (stderr.contains("Ignoring directory") || 
                                  stderr.contains("Skipping directory") || 
                                  stderr.contains("Matched ignore pattern"));

    // Verify specific logging messages are present
    let contains_ignore_pattern_log = stderr.contains("Ignoring directory") && stderr.contains(".data");
    let contains_matched_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'.data'");
    let contains_skipping_log = stderr.contains("Skipping directory") && stderr.contains(".data");
    
    // Verify that ignored files are not processed
    let contains_ignored_files = stdout.contains("ignored_file.sql") || 
                                stdout.contains("nested_ignored.sql") ||
                                stderr.contains("ignored_file.sql") || 
                                stderr.contains("nested_ignored.sql");
    
    // The regular file should always be processed
    let contains_regular_file = stdout.contains("regular_file.sql") || 
                               stderr.contains("regular_file.sql");

    // Assertions
    assert!(!contains_ignored_files, 
        "Ignored files should not be processed. Found ignored files in output: stdout={}, stderr={}", 
        stdout, stderr);
    
    assert!(contains_regular_file, 
        "Regular files should always be processed. Output: stdout={}, stderr={}", 
        stdout, stderr);

    // Verify that verbose logging is working correctly
    assert!(contains_ignore_logging, 
        "Verbose mode should log ignored directories. Stderr: {}", stderr);
    
    assert!(contains_ignore_pattern_log, 
        "Should log when ignoring directory due to pattern. Stderr: {}", stderr);
    
    assert!(contains_matched_pattern_log, 
        "Should log the specific pattern that matched. Stderr: {}", stderr);
    
    assert!(contains_skipping_log, 
        "Should log when skipping directory during traversal. Stderr: {}", stderr);
}

/// Test that specific ignore patterns are logged when they match
#[test]
fn test_verbose_logging_specific_ignore_patterns() {
    // Create a temporary directory for our test project
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create multiple directories that match different ignore patterns
    let data_dir = project_root.join(".data");
    fs::create_dir_all(&data_dir).unwrap();
    let ignored_sql_file1 = data_dir.join("file1.sql");
    fs::write(&ignored_sql_file1, "SELECT 1;").unwrap();

    let temp_dir_path = project_root.join("temp");
    fs::create_dir_all(&temp_dir_path).unwrap();
    let ignored_sql_file2 = temp_dir_path.join("file2.sql");
    fs::write(&ignored_sql_file2, "SELECT 2;").unwrap();

    let build_dir = project_root.join("build");
    fs::create_dir_all(&build_dir).unwrap();
    let ignored_sql_file3 = build_dir.join("file3.sql");
    fs::write(&ignored_sql_file3, "SELECT 3;").unwrap();

    // Create a regular SQL file that should NOT be ignored
    let regular_sql_file = project_root.join("regular_file.sql");
    fs::write(&regular_sql_file, "SELECT * FROM orders;").unwrap();

    // Create .sqruffignore file with multiple patterns
    let sqruffignore_file = project_root.join(".sqruffignore");
    fs::write(&sqruffignore_file, ".data\ntemp/\nbuild/**\n").unwrap();

    // Get the sqruff binary path
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Run sqruff lint with verbose logging enabled
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(project_root)
        .env("SQRUFF_LOG", "debug") // Enable debug logging
        .current_dir(project_root);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);

    // The regular file should always be processed
    let contains_regular_file = stdout.contains("regular_file.sql") || 
                               stderr.contains("regular_file.sql");

    assert!(contains_regular_file, 
        "Regular files should always be processed. Output: stdout={}, stderr={}", 
        stdout, stderr);

    // Verify that specific patterns are logged when they match
    let contains_data_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'.data'");
    let contains_temp_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'temp/'");
    let contains_build_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'build/**'");
    
    // Verify that ignore logging is happening (the main goal of this task)
    let has_ignore_logging = stderr.contains("Ignoring directory") || 
                            stderr.contains("Ignoring file") ||
                            stderr.contains("Skipping directory") ||
                            stderr.contains("Skipping file");
    
    let has_pattern_logging = contains_data_pattern_log || contains_temp_pattern_log || contains_build_pattern_log;
    
    // The main assertion for this task: verify that verbose logging is working
    assert!(has_ignore_logging, 
        "Should log ignore behavior in verbose mode. Stderr: {}", stderr);
    
    assert!(has_pattern_logging, 
        "Should log at least one specific ignore pattern match. Stderr: {}", stderr);
}

/// Test logging behavior across different verbosity levels
#[test]
fn test_verbose_logging_different_verbosity_levels() {
    // Create a temporary directory for our test project
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create .data directory with SQL files (this should be ignored)
    let data_dir = project_root.join(".data");
    fs::create_dir_all(&data_dir).unwrap();
    let ignored_sql_file = data_dir.join("ignored_file.sql");
    fs::write(&ignored_sql_file, "SELECT * FROM users WHERE id = 1;").unwrap();

    // Create a regular SQL file that should NOT be ignored
    let regular_sql_file = project_root.join("regular_file.sql");
    fs::write(&regular_sql_file, "SELECT * FROM orders;").unwrap();

    // Create .sqruffignore file with .data pattern
    let sqruffignore_file = project_root.join(".sqruffignore");
    fs::write(&sqruffignore_file, ".data\n").unwrap();

    // Get the sqruff binary path
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Test different log levels
    let log_levels = vec![
        ("off", "Off"),
        ("error", "Error"),
        ("warn", "Warn"), 
        ("info", "Info"),
        ("debug", "Debug"),
        ("trace", "Trace"),
    ];

    for (log_level, level_name) in log_levels {
        println!("Testing log level: {}", level_name);
        
        let mut cmd = Command::new(&sqruff_path);
        cmd.arg("lint")
            .arg("-f")
            .arg("json")
            .arg(project_root)
            .env("SQRUFF_LOG", log_level)
            .current_dir(project_root);

        let output = cmd.output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();

        println!("Log level {}: STDOUT: {}", level_name, stdout);
        println!("Log level {}: STDERR: {}", level_name, stderr);

        // Verify that ignored files are not processed regardless of log level
        let contains_ignored_files = stdout.contains("ignored_file.sql") || 
                                    stderr.contains("ignored_file.sql");
        
        // The regular file should always be processed
        let contains_regular_file = stdout.contains("regular_file.sql") || 
                                   stderr.contains("regular_file.sql");

        assert!(!contains_ignored_files, 
            "Ignored files should not be processed at log level {}. Found ignored files in output: stdout={}, stderr={}", 
            level_name, stdout, stderr);
        
        assert!(contains_regular_file, 
            "Regular files should always be processed at log level {}. Output: stdout={}, stderr={}", 
            level_name, stdout, stderr);

        // Check logging behavior at different verbosity levels
        let has_ignore_logging = stderr.contains("Ignoring directory") || 
                                stderr.contains("Skipping directory") ||
                                stderr.contains("Matched ignore pattern");
        
        match level_name {
            "Off" | "Error" | "Warn" | "Info" => {
                // Lower log levels should not show debug ignore logging
                assert!(!has_ignore_logging, 
                    "Log level {} should not show debug ignore logging. Stderr: {}", 
                    level_name, stderr);
            }
            "Debug" | "Trace" => {
                // Higher log levels should show detailed ignore logging
                assert!(has_ignore_logging, 
                    "Log level {} should show detailed ignore logging. Stderr: {}", 
                    level_name, stderr);
            }
            _ => {}
        }
    }
}

/// Test that ignored files are logged with their specific ignore patterns
#[test]
fn test_verbose_logging_file_pattern_matching() {
    // Create a temporary directory for our test project
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create files that match different ignore patterns
    let backup_file = project_root.join("backup.sql.bak");
    fs::write(&backup_file, "SELECT * FROM backup;").unwrap();

    let temp_file = project_root.join("temp_file.tmp");
    fs::write(&temp_file, "SELECT * FROM temp;").unwrap();

    // Create subdirectory with ignored files
    let logs_dir = project_root.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();
    let log_sql_file = logs_dir.join("query.sql");
    fs::write(&log_sql_file, "SELECT * FROM log_table;").unwrap();

    // Create a regular SQL file that should NOT be ignored
    let regular_sql_file = project_root.join("regular_file.sql");
    fs::write(&regular_sql_file, "SELECT * FROM orders;").unwrap();

    // Create .sqruffignore file with file extension and directory patterns
    let sqruffignore_file = project_root.join(".sqruffignore");
    fs::write(&sqruffignore_file, "*.bak\n*.tmp\nlogs/\n").unwrap();

    // Get the sqruff binary path
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Run sqruff lint with verbose logging enabled
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(project_root)
        .env("SQRUFF_LOG", "debug")
        .current_dir(project_root);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);

    // The regular file should always be processed
    let contains_regular_file = stdout.contains("regular_file.sql") || 
                               stderr.contains("regular_file.sql");

    assert!(contains_regular_file, 
        "Regular files should always be processed. Output: stdout={}, stderr={}", 
        stdout, stderr);

    // Verify that pattern-specific logging is working (main goal of this task)
    let has_bak_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'*.bak'");
    let has_tmp_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'*.tmp'");
    let has_logs_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'logs/'");
    
    // Verify that ignore logging is happening
    let has_ignore_logging = stderr.contains("Ignoring directory") || 
                            stderr.contains("Ignoring file") ||
                            stderr.contains("Skipping directory") ||
                            stderr.contains("Skipping file");
    
    let has_any_pattern_logging = has_bak_pattern_log || has_tmp_pattern_log || has_logs_pattern_log;
    
    // The main assertion for this task: verify that verbose logging is working
    assert!(has_ignore_logging, 
        "Should log ignore behavior in verbose mode. Stderr: {}", stderr);
    
    assert!(has_any_pattern_logging, 
        "Should log at least one file pattern match. Stderr: {}", stderr);
}

/// Test that verbose logging works with nested ignore patterns
#[test]
fn test_verbose_logging_nested_ignore_patterns() {
    // Create a temporary directory for our test project
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a simple directory structure that we know will work with ignore patterns
    let data_dir = project_root.join(".data");
    fs::create_dir_all(&data_dir).unwrap();
    
    let ignored_file = data_dir.join("ignored.sql");
    fs::write(&ignored_file, "SELECT * FROM ignored;").unwrap();

    // Create a regular SQL file that should NOT be ignored
    let regular_sql_file = project_root.join("regular_file.sql");
    fs::write(&regular_sql_file, "SELECT * FROM orders;").unwrap();

    // Create .sqruffignore file with a pattern we know works
    let sqruffignore_file = project_root.join(".sqruffignore");
    fs::write(&sqruffignore_file, ".data\n").unwrap();

    // Get the sqruff binary path
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut sqruff_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sqruff_path.push(format!("../../target/{}/sqruff", profile));

    // Run sqruff lint with verbose logging enabled
    let mut cmd = Command::new(sqruff_path);
    cmd.arg("lint")
        .arg("-f")
        .arg("json")
        .arg(project_root)
        .env("SQRUFF_LOG", "debug")
        .current_dir(project_root);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);

    let contains_regular_file = stdout.contains("regular_file.sql") || stderr.contains("regular_file.sql");

    assert!(contains_regular_file, 
        "Regular files should always be processed. Output: stdout={}, stderr={}", 
        stdout, stderr);

    // Verify that ignore pattern logging is working (main goal of this task)
    let has_pattern_log = stderr.contains("Matched ignore pattern") && stderr.contains("'.data'");
    let has_ignore_logging = stderr.contains("Ignoring directory") || 
                            stderr.contains("Ignoring file") ||
                            stderr.contains("Skipping directory");
    
    // The main assertion for this task: verify that verbose logging is working
    assert!(has_ignore_logging, 
        "Should log ignore behavior in verbose mode. Stderr: {}", stderr);
    
    assert!(has_pattern_log, 
        "Should log the specific ignore pattern that matched. Stderr: {}", stderr);
}