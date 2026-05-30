use fancy_regex::Regex;
use hashbrown::HashMap;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::{
    RawFileSlice, TemplateSliceKind, TemplatedFile, TemplatedFileSlice,
};

use crate::config::FluffConfig;
use crate::templaters::{
    PlaceholderStyle, ProcessingMode, Templater, TemplaterError, TemplaterInput, TemplaterOutput,
    source_id_name,
};

#[derive(Default)]
pub struct PlaceholderTemplater;

pub fn get_known_styles() -> HashMap<&'static str, Regex> {
    PlaceholderStyle::all()
        .iter()
        .map(|style| (style.as_str(), style.regex()))
        .collect()
}

const NO_PARAM_OR_STYLE: &str =
    "No param_regex nor param_style was provided to the placeholder templater.";

impl PlaceholderTemplater {
    fn derive_style(&self, config: &FluffConfig) -> Result<Regex, SQLFluffUserError> {
        let config = &config.templater().placeholder;
        match (&config.param_regex, config.param_style) {
            (Some(_), Some(_)) => Err(SQLFluffUserError::new(
                "Both param_regex and param_style were provided to the placeholder templater."
                    .to_string(),
            )),
            (None, None) => Err(SQLFluffUserError::new(NO_PARAM_OR_STYLE.to_string())),
            (Some(param_regex), None) => {
                let regex = Regex::new(param_regex).map_err(|e| {
                    SQLFluffUserError::new(format!("Invalid regex for param_regex: {e}"))
                })?;
                Ok(regex)
            }
            (None, Some(style)) => Ok(style.regex()),
        }
    }

    fn process_single(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let mut template_slices = vec![];
        let mut raw_slices = vec![];
        let mut last_pos_raw = 0usize;
        let mut last_pos_templated = 0;
        let mut out_str = "".to_string();

        // when the param has no name, use a 1-based index
        let mut param_counter = 1;
        let regex = self.derive_style(config)?;

        let template_config = &config.templater().placeholder;

        for cap in regex.captures_iter(in_str) {
            let cap = cap.unwrap();
            let span = cap.get(0).unwrap().range();

            let param_name = if let Some(name) = cap.name("param_name") {
                name.as_str().to_string()
            } else {
                let name = param_counter.to_string();
                param_counter += 1;
                name
            };

            let last_literal_length = span.start - last_pos_raw;
            let replacement = template_config
                .values
                .get(&param_name)
                .map_or_else(|| param_name.clone(), |value| value.as_replacement());

            // Add the literal to the slices
            template_slices.push(TemplatedFileSlice::new(
                TemplateSliceKind::Literal,
                last_pos_raw..span.start,
                last_pos_templated..last_pos_templated + last_literal_length,
            ));

            raw_slices.push(RawFileSlice::new(
                in_str[last_pos_raw..span.start].to_string(),
                TemplateSliceKind::Literal,
                last_pos_raw,
                None,
                None,
            ));

            out_str.push_str(&in_str[last_pos_raw..span.start]);

            // Add the current replaced element
            let start_template_pos = last_pos_templated + last_literal_length;
            template_slices.push(TemplatedFileSlice::new(
                TemplateSliceKind::Templated,
                span.clone(),
                start_template_pos..start_template_pos + replacement.len(),
            ));

            let raw_file_slice = RawFileSlice::new(
                in_str[span.clone()].to_string(),
                TemplateSliceKind::Templated,
                span.start,
                None,
                None,
            );
            raw_slices.push(raw_file_slice);

            out_str.push_str(&replacement);

            // Update the indexes
            last_pos_raw = span.end;
            last_pos_templated = start_template_pos + replacement.len();
        }

        // Add the last literal, if any
        if in_str.len() > last_pos_raw {
            template_slices.push(TemplatedFileSlice::new(
                TemplateSliceKind::Literal,
                last_pos_raw..in_str.len(),
                last_pos_templated..last_pos_templated + (in_str.len() - last_pos_raw),
            ));

            let raw_file_slice = RawFileSlice::new(
                in_str[last_pos_raw..].to_string(),
                TemplateSliceKind::Literal,
                last_pos_raw,
                None,
                None,
            );
            raw_slices.push(raw_file_slice);

            out_str.push_str(&in_str[last_pos_raw..]);
        }

        let templated_file = TemplatedFile::new(
            in_str.to_string(),
            f_name.to_string(),
            Some(out_str),
            Some(template_slices),
            Some(raw_slices),
        )
        .unwrap();

        Ok(templated_file)
    }
}

impl Templater for PlaceholderTemplater {
    fn name(&self) -> &'static str {
        "placeholder"
    }

    fn description(&self) -> &'static str {
        r#"Libraries such as SQLAlchemy or Psycopg use different parameter placeholder styles to mark where a parameter has to be inserted in the query.

For example a query in SQLAlchemy can look like this:

```sql
SELECT * FROM table WHERE id = :myid
```

At runtime :myid will be replace by a value provided by the application and escaped as needed, but this is not standard SQL and cannot be parsed as is.

In order to parse these queries is then necessary to replace these placeholders with sample values, and this is done with the placeholder templater.

Placeholder templating can be enabled in the config using:

```ini
[sqruff]
templater = placeholder
```

A few common styles are supported:

```sql
 -- colon
 WHERE bla = :my_name

 -- colon_nospaces
 -- (use with caution as more prone to false positives)
 WHERE bla = table:my_name

 -- colon_optional_quotes
 SELECT :"column" FROM :table WHERE bla = :'my_name'

 -- numeric_colon
 WHERE bla = :2

 -- pyformat
 WHERE bla = %(my_name)s

 -- dollar
 WHERE bla = $my_name or WHERE bla = ${my_name}

 -- question_mark
 WHERE bla = ?

 -- numeric_dollar
 WHERE bla = $3 or WHERE bla = ${3}

 -- percent
 WHERE bla = %s

 -- ampersand
 WHERE bla = &s or WHERE bla = &{s} or USE DATABASE MARK_{ENV}

 -- apache_camel
 WHERE bla = :#${qwe}

 -- at
 WHERE bla = @my_name
```

The can be configured by setting `param_style` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_style = colon
my_name = 'john'
```

then you can set sample values for each parameter, like my_name above. Notice that the value needs to be escaped as it will be replaced as a string during parsing. When the sample values aren't provided, the templater will use parameter names themselves by default.

When parameters are positional, like question_mark, then their name is simply the order in which they appear, starting with 1.

```ini
[sqruff:templater:placeholder]
param_style = question_mark
1 = 'john'
```

In case you nbeed a parameter style different from the ones provided, you can set `param_regex` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_regex = __(?P<param_name>[\w_]+)__
my_name = 'john'
```

N.B. quotes around param_regex in the config are interpreted literally by the templater. e.g. param_regex='__(?P<param_name>[w_]+)__' matches '__some_param__' not __some_param__

the named parameter param_name will be used as the key to replace, if missing, the parameter is assumed to be positional and numbers are used instead.

Also consider making a pull request to the project to have your style added, it may be useful to other people and simplify your configuration."#
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::Parallel
    }

    fn process(
        &self,
        files: &[TemplaterInput<'_>],
        config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
        files
            .iter()
            .map(|file| {
                let fname = source_id_name(file.source_id);
                self.process_single(file.source, &fname, config)
                    .map(TemplaterOutput::Rendered)
                    .map_err(TemplaterError::Failed)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::api::{Engine, EngineOptions, ParseErrors, Source, SourceId};
    use std::borrow::Cow;

    type PlaceholderCase<'a> = (&'a str, &'a str, &'a str, Vec<(&'a str, &'a str)>);

    fn process_one(
        templater: &PlaceholderTemplater,
        in_str: &str,
        name: &str,
        config: &FluffConfig,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let source_id = SourceId::Virtual(name.to_string());
        templater
            .process(
                &[TemplaterInput {
                    source: in_str,
                    source_id: &source_id,
                }],
                config,
            )
            .into_iter()
            .next()
            .unwrap()
            .map(|output| match output {
                TemplaterOutput::Rendered(file) => file,
                TemplaterOutput::Skipped(reason) => {
                    panic!("placeholder templater skipped: {}", reason.message)
                }
            })
            .map_err(TemplaterError::into_user_error)
    }

    #[test]
    /// Test the templaters when nothing has to be replaced.
    fn test_templater_no_replacement() {
        let templater = PlaceholderTemplater {};
        let in_str = "SELECT * FROM {{blah}} WHERE %(gnepr)s OR e~':'";
        let config = FluffConfig::try_from_source(
            "
[sqruff:templater:placeholder]
param_style = colon",
            None,
        )
        .unwrap();
        let out_str = process_one(&templater, in_str, "test.sql", &config).unwrap();
        let out = out_str.templated();
        assert_eq!(in_str, out)
    }

    #[test]
    fn test_all_the_known_styles() {
        // in, param_style, expected_out, values
        let cases: [PlaceholderCase<'_>; 19] = [
            (
                "SELECT * FROM f, o, o WHERE a < 10\n\n",
                "colon",
                "SELECT * FROM f, o, o WHERE a < 10\n\n",
                vec![],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = :user_id AND date > :start_date
"#,
                "colon",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = 42 AND date > '2020-01-01'
"#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = :user_id AND date > :start_date"#,
                "colon",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = 42 AND date > '2020-01-01'"#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN :city_ids
AND date > '2020-10-01'
            "#,
                "colon",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3)
AND date > '2020-10-01'
            "#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = @user_id AND date > @start_date
"#,
                "at",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = 42 AND date > '2020-01-01'
"#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = @user_id AND date > @start_date"#,
                "at",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE userid = 42 AND date > '2020-01-01'"#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN @city_ids
AND date > '2020-10-01'
            "#,
                "at",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3)
AND date > '2020-10-01'
            "#,
                vec![
                    ("user_id", "42"),
                    ("start_date", "'2020-01-01'"),
                    ("city_ids", "(1, 2, 3)"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data:table_suffix
"#,
                "colon_nospaces",
                r#"
SELECT user_mail, city_id
FROM users_data42
"#,
                vec![("table_suffix", "42")],
            ),
            (
                // Postgres uses double-colons for type casts, see
                // https://www.postgresql.org/docs/current/sql-expressions.html#SQL-SYNTAX-TYPE-CASTS
                // This test ensures we don't confuse them with colon placeholders.
                r#"
SELECT user_mail, city_id, joined::date
FROM users_date:table_suffix
"#,
                "colon_nospaces",
                r#"
SELECT user_mail, city_id, joined::date
FROM users_date42
"#,
                vec![("table_suffix", "42")],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN ?
AND date > ?
            "#,
                "question_mark",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
            "#,
                vec![("1", "(1, 2, 3, 45)"), ("2", "'2020-10-01'")],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN :1
AND date > :45
            "#,
                "numeric_colon",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
            "#,
                vec![("1", "(1, 2, 3, 45)"), ("45", "'2020-10-01'")],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN %(city_id)s
AND date > %(date)s
AND someflag = %(someflag)s
LIMIT %(limit)s
            "#,
                "pyformat",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
AND someflag = false
LIMIT 15
            "#,
                vec![
                    ("city_id", "(1, 2, 3, 45)"),
                    ("date", "'2020-10-01'"),
                    ("limit", "15"),
                    ("someflag", "false"),
                ],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN $city_id
AND date > $date
OR date = ${date}
            "#,
                "dollar",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
OR date = '2020-10-01'
            "#,
                vec![("city_id", "(1, 2, 3, 45)"), ("date", "'2020-10-01'")],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN $12
AND date > $90
            "#,
                "numeric_dollar",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
            "#,
                vec![("12", "(1, 2, 3, 45)"), ("90", "'2020-10-01'")],
            ),
            (
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN %s
AND date > %s
            "#,
                "percent",
                r#"
SELECT user_mail, city_id
FROM users_data
WHERE (city_id) IN (1, 2, 3, 45)
AND date > '2020-10-01'
            "#,
                vec![("1", "(1, 2, 3, 45)"), ("2", "'2020-10-01'")],
            ),
            (
                r#"
USE DATABASE &{env}_MARKETING;
USE SCHEMA &&EMEA;
SELECT user_mail, city_id
FROM users_data
WHERE userid = &user_id AND date > &{start_date}
            "#,
                "ampersand",
                r#"
USE DATABASE PRD_MARKETING;
USE SCHEMA &&EMEA;
SELECT user_mail, city_id
FROM users_data
WHERE userid = 42 AND date > '2021-10-01'
            "#,
                vec![
                    ("env", "PRD"),
                    ("user_id", "42"),
                    ("start_date", "'2021-10-01'"),
                ],
            ),
            (
                "USE ${flywaydatabase}.test_schema;",
                "flyway_var",
                "USE test_db.test_schema;",
                vec![("flywaydatabase", "test_db")],
            ),
            (
                "SELECT metadata$filename, $1 FROM @stg_data_export_${env_name};",
                "flyway_var",
                "SELECT metadata$filename, $1 FROM @stg_data_export_staging;",
                vec![("env_name", "staging")],
            ),
            (
                "SELECT metadata$filename, $1 FROM @stg_data_export_${env_name};",
                "flyway_var",
                "SELECT metadata$filename, $1 FROM @stg_data_export_env_name;",
                vec![],
            ),
        ];

        for (in_str, param_style, expected_out, values) in cases {
            let config = FluffConfig::try_from_source(
                format!(
                    r#"
[sqruff:templater:placeholder]
param_style = {}
{}
"#,
                    param_style,
                    values
                        .iter()
                        .map(|(k, v)| format!("{} = {}", k, v))
                        .collect::<Vec<String>>()
                        .join("\n")
                )
                .as_str(),
                None,
            )
            .unwrap();
            let templater = PlaceholderTemplater {};
            let out_str = process_one(&templater, in_str, "test.sql", &config).unwrap();
            let out = out_str.templated();
            assert_eq!(expected_out, out)
        }
    }

    #[test]
    /// Test the error raised when config is incomplete, as in no param_regex
    /// nor param_style.
    fn test_templater_setup_none() {
        let config = FluffConfig::try_from_source("", None).unwrap();
        let templater = PlaceholderTemplater {};
        let in_str = "SELECT 2+2";
        let out_str = process_one(&templater, in_str, "test.sql", &config);

        assert!(out_str.is_err());
        assert_eq!(
            out_str.err().unwrap().value,
            "No param_regex nor param_style was provided to the placeholder templater."
        );
    }

    #[test]
    /// Test the error raised when both param_regex and param_style are
    /// provided.
    fn test_templater_setup_both_provided() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff:templater:placeholder]
param_regex = __(?P<param_name>[\w_]+)__
param_style = colon
            "#,
            None,
        )
        .unwrap();
        let templater = PlaceholderTemplater {};
        let in_str = "SELECT 2+2";
        let out_str = process_one(&templater, in_str, "test.sql", &config);

        assert!(out_str.is_err());
        assert_eq!(
            out_str.err().unwrap().value,
            "Both param_regex and param_style were provided to the placeholder templater."
        );
    }

    #[test]
    /// Test custom regex templating.
    fn test_templater_custom_regex() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff:templater:placeholder]
param_regex = __(?P<param_name>[\w_]+)__
my_name = john
"#,
            None,
        )
        .unwrap();
        let templater = PlaceholderTemplater {};
        let in_str = "SELECT bla FROM blob WHERE id = __my_name__";
        let out_str = process_one(&templater, in_str, "test", &config).unwrap();
        let out = out_str.templated();
        assert_eq!("SELECT bla FROM blob WHERE id = john", out)
    }

    #[test]
    /// Test the exception raised when parameter styles is unknown.
    fn test_templater_styles_not_existing() {
        let err = FluffConfig::try_from_source(
            r#"
[sqruff:templater:placeholder]
param_style = unknown
            "#,
            None,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Unknown placeholder style 'unknown'")
        );
    }

    #[test]
    /// Test the linter fully with this templater.
    fn test_templater_placeholder() {
        let config = FluffConfig::try_from_source(
            r#"
[sqruff]
dialect = ansi
templater = placeholder
rules = all

[sqruff:templater:placeholder]
param_style = percent
"#,
            None,
        )
        .unwrap();
        let sql = "SELECT a,b FROM users WHERE a = %s";

        let result = Engine::new(
            config,
            EngineOptions {
                parse_errors: ParseErrors::Suppress,
            },
        )
        .unwrap()
        .fix_source(Source {
            id: SourceId::Virtual("test.sql".into()),
            text: Cow::Borrowed(sql),
        })
        .unwrap()
        .fixed_source
        .unwrap();

        assert_eq!(result, "SELECT\n    a,\n    b\nFROM users\nWHERE a = %s\n");
    }
}
