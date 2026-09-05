use hashbrown::HashSet;

use expect_test::expect_file;
use glob::glob;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::Lexer;
use sqruff_lib_core::parser::segments::Tables;

fn main() {
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
        let config_path = templater_setup.join(".sqruff");
        let config = std::fs::read_to_string(&config_path).unwrap();
        let config = FluffConfig::from_source(&config, Some(&config_path));

        let templater = match Linter::get_templater(&config) {
            Ok(t) => t,
            Err(e) => {
                println!(
                    "Skipping templater test for {:?}: {}",
                    templater_setup.file_name().unwrap(),
                    e
                );
                continue;
            }
        };

        // Check root fixture files and models nested below `model_directory`.
        // Macro directories are intentionally excluded because their SQL files
        // are inputs to the templater rather than parse expectations.
        let sql_patterns = [
            format!("{}/*.sql", templater_setup.to_str().unwrap()),
            format!(
                "{}/model_directory/**/*.sql",
                templater_setup.to_str().unwrap()
            ),
        ];
        for sql_file in sql_patterns
            .iter()
            .flat_map(|pattern| glob(pattern).unwrap())
        {
            let sql_file = sql_file.unwrap();
            let yaml_file = sql_file.with_extension("yml");
            let yaml_file = std::path::absolute(yaml_file).unwrap();

            let actual = {
                let dialect = config.get_dialect();
                let sql = std::fs::read_to_string(&sql_file).unwrap();
                let tables = Tables::default();
                let lexer = Lexer::from(dialect);
                let parser = Parser::from(dialect);

                let file_name = sql_file.to_string_lossy();
                let templated_file = templater
                    .process(&[(&sql, &file_name)], &config, &None)
                    .into_iter()
                    .next()
                    .unwrap()
                    .unwrap();

                let (tokens, errors) = lexer.lex(&tables, templated_file);
                assert!(errors.is_empty());

                let parsed = parser.parse(&tables, &tokens).unwrap();
                let tree = parsed.unwrap();
                let tree = tree.to_serialised(true, true);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml_file].assert_eq(&actual);
        }
    }
}
