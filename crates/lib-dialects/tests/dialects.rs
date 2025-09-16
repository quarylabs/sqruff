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
        .collect::<HashSet<std::path::PathBuf>>();
    println!("{dialects_dirs:?}");

    // check if all dialects have a corresponding folder
    for dialect in &dialects {
        let dialect_dir = dialects_dir.join(dialect);
        assert!(dialects_dirs.contains(&dialect_dir));
    }

    if arg_dialect.is_none() {
        assert_eq!(dialects_dirs.len(), dialects.len());
    }

    // Go through each of the dialects and check if the files are present
    for dialect_name in &dialects {
        let dialect_kind = DialectKind::from_str(dialect_name).unwrap();
        let Some(dialect) = kind_to_dialect(&dialect_kind) else {
            println!("{dialect_name} disabled");
            continue;
        };

        let path = format!("test/fixtures/dialects/{dialect_name}/*/*.sql");
        let files = glob::glob(&path).unwrap().flatten().collect_vec();

        println!("For dialect: {dialect_name}, found {} files", files.len());

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
