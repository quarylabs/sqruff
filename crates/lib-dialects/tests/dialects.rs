use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use ahash::HashSet;
use expect_test::expect_file;
use itertools::Itertools;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::value::Value;
use sqruff_lib_dialects::kind_to_dialect;
use strum::IntoEnumIterator;

fn check_no_unparsable_segments(tree: &ErasedSegment) -> Vec<String> {
    tree.recursive_crawl_all(false)
        .into_iter()
        .filter(|segment| segment.is_type(SyntaxKind::Unparsable))
        .map(|segment| {
            format!(
                "Unparsable segment found: {} at position {:?}",
                segment.raw(),
                segment.get_position_marker()
            )
        })
        .collect()
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<String>>();

    let mut arg_dialect = None;

    if args.len() == 1 {
        arg_dialect = Some(DialectKind::from_str(&args[0]).unwrap());
    }

    let dialects = DialectKind::iter()
        .filter(|dialect| arg_dialect.is_none() || &arg_dialect.unwrap() == dialect)
        .map(|dialect| dialect.as_ref().to_string())
        .collect::<HashSet<String>>();
    println!("{dialects:?}");

    // list folders in the dialects directory
    let dialects_dir = std::path::Path::new("test/fixtures/dialects");
    let dialects_dirs = dialects_dir
        .read_dir()
        .unwrap()
        .filter_map(|entry| {
            // if file ignore
            if entry.as_ref().unwrap().file_type().unwrap().is_file() {
                return None;
            }
            Some(entry.unwrap().path())
        })
        .collect::<HashSet<PathBuf>>();
    println!("{dialects_dirs:?}");

    // check if all dialects have a corresponding folder
    for dialect in &dialects {
        let dialect_dir = dialects_dir.join(dialect);
        assert!(dialects_dirs.contains(&dialect_dir), "{}", dialect);
    }

    if arg_dialect.is_none() {
        assert_eq!(dialects_dirs.len(), dialects.len());
    }

    // Go through each of the dialects and check if the files are present
    for dialect_name in &dialects {
        let dialect_kind = DialectKind::from_str(dialect_name).unwrap();

        let path = format!("test/fixtures/dialects/{dialect_name}/*/*.sql");
        let files = glob::glob(&path).unwrap().flatten().collect_vec();

        println!("For dialect: {dialect_name}, found {} files", files.len());

        // Group files by their parent directory (subfolder)
        let files_by_subfolder: HashMap<PathBuf, Vec<PathBuf>> =
            files.into_iter().fold(HashMap::new(), |mut acc, file| {
                let subfolder = file.parent().unwrap().to_path_buf();
                acc.entry(subfolder).or_default().push(file);
                acc
            });

        // Process each subfolder with its own config
        for (subfolder, files) in files_by_subfolder {
            // Check for config.toml in the subfolder
            let config_path = subfolder.join("config.toml");
            let config: Option<Value> = if config_path.exists() {
                let config_str = std::fs::read_to_string(&config_path).unwrap();
                let toml_value: toml::Value = toml::from_str(&config_str).unwrap();
                Some(toml_to_value(&toml_value))
            } else {
                None
            };

            let Some(dialect) = kind_to_dialect(&dialect_kind, config.as_ref()) else {
                println!("{dialect_name} disabled");
                continue;
            };

            files.par_iter().for_each(|file| {
                let _panic = helpers::enter_panic(file.display().to_string());

                let yaml = file.with_extension("yml");
                let yaml = std::path::absolute(yaml).unwrap();

                let actual = {
                    let sql = std::fs::read_to_string(file).unwrap();
                    let tables = Tables::default();
                    let lexer = Lexer::from(&dialect);
                    let parser = Parser::from(&dialect);
                    let tokens = lexer.lex(&tables, sql);
                    assert!(tokens.1.is_empty());

                    let parsed = parser.parse(&tables, &tokens.0).unwrap();
                    let tree = parsed.unwrap();

                    // Check for unparsable segments
                    let unparsable_segments = check_no_unparsable_segments(&tree);
                    if !unparsable_segments.is_empty() {
                        panic!(
                            "Found unparsable segments in {}: {}",
                            file.display(),
                            unparsable_segments.join(", ")
                        );
                    }

                    let tree = tree.to_serialised(true, true);

                    serde_yaml::to_string(&tree).unwrap()
                };

                expect_file![yaml].assert_eq(&actual);
            });
        }
    }
}

/// Convert a toml::Value to sqruff_lib_core::value::Value
fn toml_to_value(toml: &toml::Value) -> Value {
    match toml {
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Integer(i) => Value::Int(*i as i32),
        toml::Value::Float(f) => Value::Float(*f),
        toml::Value::String(s) => Value::String(s.clone().into_boxed_str()),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_to_value).collect()),
        toml::Value::Table(table) => {
            let map = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_value(v)))
                .collect();
            Value::Map(map)
        }
        toml::Value::Datetime(_) => Value::None,
    }
}
