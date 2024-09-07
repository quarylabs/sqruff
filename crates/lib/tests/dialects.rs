use ahash::HashSet;
use expect_test::expect_file;
use itertools::Itertools;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use sqruff_lib::core::config::{FluffConfig, Value};
use sqruff_lib::core::dialects::init::DialectKind;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::core::parser::segments::base::{ErasedSegment, Tables};
use sqruff_lib::helpers;
use strum::IntoEnumIterator;

#[derive(Default)]
pub struct Args {
    list: bool,
    ignored: bool,
    no_capture: bool,
}

impl Args {
    fn parse_args(&mut self, mut iter: impl Iterator<Item = String>) {
        while let Some(arg) = iter.next() {
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
    for dialect in &dialects {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map(
                        [("dialect".into(), Value::String(Box::from(dialect.to_string())))].into(),
                    ),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let path = format!("test/fixtures/dialects/{}/*.sql", dialect);
        let files = glob::glob(&path).unwrap().flatten().collect_vec();

        println!("For dialect: {}, found {} files", dialect, files.len());

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&linter, &sql);
                let tree = tree.to_serialised(true, true);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}

fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
    let tables = Tables::default();
    let parsed = linter.parse_string(&tables, sql, None, None, None).unwrap();
    parsed.tree.unwrap()
}
