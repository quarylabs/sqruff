use std::str::FromStr;

use ahash::HashSet;
use expect_test::expect_file;
use itertools::Itertools;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::helpers;
use sqruff_lib_core::parser::lexer::{Lexer, StringOrTemplate};
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_core::parser::segments::base::Tables;
use sqruff_lib_dialects::kind_to_dialect;
use strum::IntoEnumIterator;

#[derive(Default)]
pub struct Args {
    list: bool,
    ignored: bool,
    no_capture: bool,
}

impl Args {
    fn parse_args(&mut self, iter: impl Iterator<Item = String>) {
        for arg in iter {
            if arg == "--" {
                continue;
            }

            match arg.as_str() {
                "--list" => self.list = true,
                "--ignored" => self.ignored = true,
                "--no-capture" => self.no_capture = true,
                _ => {}
            }
        }
    }
}

fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1));

    // FIXME: improve support for nextest
    if args.list {
        if !args.ignored {
            println!("rules: test");
        }

        return;
    }

    let dialects = DialectKind::iter()
        .map(|dialect| dialect.as_ref().to_string())
        .collect::<HashSet<String>>();
    println!("{:?}", dialects);

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
    println!("{:?}", dialects_dirs);

    // check if all dialects have a corresponding folder
    for dialect in &dialects {
        let dialect_dir = dialects_dir.join(dialect);
        assert!(dialects_dirs.contains(&dialect_dir));
    }
    assert_eq!(dialects_dirs.len(), dialects.len());

    // Go through each of the dialects and check if the files are present
    for dialect_name in &dialects {
        let dialect_kind = DialectKind::from_str(dialect_name).unwrap();
        let Some(dialect) = kind_to_dialect(&dialect_kind) else {
            println!("{} disabled", dialect_name);
            continue;
        };

        let path = format!("test/fixtures/dialects/{}/*.sql", dialect_name);
        let files = glob::glob(&path).unwrap().flatten().collect_vec();

        println!("For dialect: {}, found {} files", dialect_name, files.len());

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tables = Tables::default();
                let lexer = Lexer::from(&dialect);
                let parser = Parser::from(&dialect);
                let tokens = lexer.lex(&tables, StringOrTemplate::String(&sql)).unwrap();
                assert!(tokens.1.is_empty());

                let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
                let tree = parsed.unwrap();
                let tree = tree.to_serialised(true, true);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
