use crate::core::config::FluffConfig;
use crate::core::linter::linting_result::LintingResult;

use super::linted_dir::LintedDir;

pub struct Linter {
    config: FluffConfig,
}

impl Linter {
    pub fn new(config: FluffConfig) -> Linter {
        Linter { config }
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &self,
        sql: String,
        f_name: Option<String>,
        fix: Option<bool>,
    ) -> LintingResult {
        // TODO Translate LintingResult
        let result = LintingResult::new();
        let path = f_name.unwrap_or_else(|| "<string input>".to_string()); 
        let linted_path = LintedDir::new(path);
        panic!("Not finished");
        // linted_path = LintedDir(fname);
        // linted_path.add(self.lint_string(string, fname=fname, fix=fix))
        // result.add(linted_path)
        // result.stop_timer()
        // return result
    }

    /// Lint a string.
    pub fn lint_string(
        &self,
        in_str: Option<String>,
        fname: Option<String>,
        fix: Option<bool>,
        config: Option<FluffConfig>,
        encoding: Option<String>){
            
            panic!("Not finished");
        }
    // ) -> LintedFile:
    //     # Sort out config, defaulting to the built in config if no override
    //     config = config or self.config
    //     # Parse the string.
    //     parsed = self.parse_string(
    //         in_str=in_str,
    //         fname=fname,
    //         config=config,
    //     )
    //     # Get rules as appropriate
    //     rule_set = self.get_ruleset(config=config)
    //     # Lint the file and return the LintedFile
    //     return self.lint_parsed(
    //         parsed,
    //         rule_set,
    //         fix=fix,
    //         formatter=self.formatter,
    //         encoding=encoding,
    //     )


}
