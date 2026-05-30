use std::io::{self, BufRead};

use sqruff_lib::api::{Engine, EngineOptions, ParseErrors, Source, SourceId};
use sqruff_lib::config::FluffConfig;
use std::borrow::Cow;

use crate::commands::{ParseArgs, ParseFormat};

pub(crate) fn run_parse(args: ParseArgs, config: FluffConfig) -> i32 {
    if args.paths.is_empty() || (args.paths.len() == 1 && args.paths[0].to_str() == Some("-")) {
        run_parse_stdin(config, args.format)
    } else {
        run_parse_files(args, config)
    }
}

fn run_parse_stdin(config: FluffConfig, format: ParseFormat) -> i32 {
    let stdin = io::stdin();
    let mut sql = String::new();

    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                sql.push_str(&line);
                sql.push('\n');
            }
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                return 1;
            }
        }
    }

    parse_and_output_tree(&sql, "<stdin>", &config, format)
}

fn run_parse_files(args: ParseArgs, config: FluffConfig) -> i32 {
    let mut exit_code = 0;

    for path in &args.paths {
        match std::fs::read_to_string(path) {
            Ok(sql) => {
                let result = parse_and_output_tree(
                    &sql,
                    path.to_string_lossy().as_ref(),
                    &config,
                    args.format,
                );
                if result != 0 {
                    exit_code = result;
                }
            }
            Err(e) => {
                eprintln!("Error reading file {}: {}", path.display(), e);
                exit_code = 1;
            }
        }
    }

    exit_code
}

fn parse_and_output_tree(
    sql: &str,
    filename: &str,
    config: &FluffConfig,
    format: ParseFormat,
) -> i32 {
    let engine = match Engine::new(
        config.clone(),
        EngineOptions {
            parse_errors: ParseErrors::Include,
        },
    ) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let source = Source {
        id: SourceId::Virtual(filename.to_string()),
        text: Cow::Borrowed(sql),
    };

    match engine.parse_source(source) {
        Ok(parsed) => {
            if let Some(tree) = &parsed.tree {
                match format {
                    ParseFormat::Json => {
                        let serialized = tree.to_serialised(false, true);
                        match serde_json::to_string_pretty(&serialized) {
                            Ok(json) => println!("{}", json),
                            Err(e) => {
                                eprintln!("Error serializing to JSON: {}", e);
                                return 1;
                            }
                        }
                    }
                    ParseFormat::Pretty => {
                        println!("Parse tree for {}:", filename);
                        println!("{}", tree.stringify(false));
                    }
                }

                // Also print any parsing violations if they exist
                if !parsed.diagnostics.is_empty() {
                    eprintln!("\nParse violations:");
                    for diagnostic in &parsed.diagnostics {
                        eprintln!("  {}", diagnostic.message);
                    }
                }

                0
            } else {
                eprintln!("Error: Failed to parse {}", filename);
                if let Some(reason) = &parsed.skipped {
                    eprintln!("Skipped: {}", reason.message);
                }
                if !parsed.diagnostics.is_empty() {
                    eprintln!("Parse violations:");
                    for diagnostic in &parsed.diagnostics {
                        eprintln!("  {}", diagnostic.message);
                    }
                }
                1
            }
        }
        Err(e) => {
            eprintln!("Error parsing {}: {}", filename, e);
            1
        }
    }
}
