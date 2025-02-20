use std::collections::HashSet;

use expect_test::expect_file;
use glob::glob;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib_core::parser::lexer::{Lexer, StringOrTemplate};
use sqruff_lib_core::parser::parser::Parser;
use sqruff_lib_core::parser::segments::base::Tables;

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

// FIXME: Simplify FluffConfig handling. It's quite chaotic right now.
fn main() {
    let mut args = Args::default();
    args.parse_args(std::env::args().skip(1));

    let templaters_folder = std::path::Path::new("test/fixtures/templaters");
    let templaters_folders = templaters_folder
        .read_dir()
        .unwrap()
        .filter_map(|entry| {
            if entry.as_ref().unwrap().file_type().unwrap().is_file() {
                return None;
            }
            Some(entry.unwrap().path())
        })
        .collect::<HashSet<std::path::PathBuf>>();

    for templater_setup in &templaters_folders {
        println!("{:?}", templater_setup);
        let config = std::fs::read_to_string(templater_setup.join(".sqruff")).unwrap();
        let config = FluffConfig::from_source(&config, None);

        // for every sql file in that folder
        for sql_file in glob(&format!("{}/*.sql", templater_setup.to_str().unwrap())).unwrap() {
            let sql_file = sql_file.unwrap();
            let yaml_file = sql_file.with_extension("yml");
            let yaml_file = std::path::absolute(yaml_file).unwrap();

            let actual = {
                let dialect = config.get_dialect();
                let sql = std::fs::read_to_string(&sql_file).unwrap();
                let tables = Tables::default();
                let lexer = Lexer::from(dialect);
                let parser = Parser::from(dialect);

                let templater = Linter::get_templater(&config);
                let templated_file = templater
                    .process(&sql, &sql_file.to_string_lossy(), &config, &None)
                    .unwrap();

                let tokens = lexer
                    .lex(&tables, StringOrTemplate::Template(templated_file))
                    .unwrap();
                assert!(tokens.1.is_empty());

                let parsed = parser.parse(&tables, &tokens.0, None).unwrap();
                let tree = parsed.unwrap();
                let tree = tree.to_serialised(true, true);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml_file].assert_eq(&actual);
        }
    }
}
