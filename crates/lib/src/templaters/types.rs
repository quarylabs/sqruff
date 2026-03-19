use fancy_regex::Regex;

#[cfg(feature = "python")]
use super::{DBT_TEMPLATER, JINJA_TEMPLATER, PYTHON_TEMPLATER};
use super::{PLACEHOLDER_TEMPLATER, RAW_TEMPLATER, Templater};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplaterKind {
    Raw,
    Placeholder,
    #[cfg(feature = "python")]
    Python,
    #[cfg(feature = "python")]
    Jinja,
    #[cfg(feature = "python")]
    Dbt,
}

impl TemplaterKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Placeholder => "placeholder",
            #[cfg(feature = "python")]
            Self::Python => "python",
            #[cfg(feature = "python")]
            Self::Jinja => "jinja",
            #[cfg(feature = "python")]
            Self::Dbt => "dbt",
        }
    }

    pub fn templater(self) -> &'static dyn Templater {
        match self {
            Self::Raw => &RAW_TEMPLATER,
            Self::Placeholder => &PLACEHOLDER_TEMPLATER,
            #[cfg(feature = "python")]
            Self::Python => &PYTHON_TEMPLATER,
            #[cfg(feature = "python")]
            Self::Jinja => &JINJA_TEMPLATER,
            #[cfg(feature = "python")]
            Self::Dbt => &DBT_TEMPLATER,
        }
    }

    pub fn available_names() -> Vec<&'static str> {
        Self::available().iter().map(|kind| kind.as_str()).collect()
    }

    pub const fn available() -> &'static [Self] {
        #[cfg(feature = "python")]
        {
            &[
                Self::Raw,
                Self::Placeholder,
                Self::Python,
                Self::Jinja,
                Self::Dbt,
            ]
        }

        #[cfg(not(feature = "python"))]
        {
            &[Self::Raw, Self::Placeholder]
        }
    }

    pub fn from_name(s: &str) -> Result<Self, String> {
        match s {
            "raw" => Ok(Self::Raw),
            "placeholder" => Ok(Self::Placeholder),
            #[cfg(feature = "python")]
            "python" => Ok(Self::Python),
            #[cfg(feature = "python")]
            "jinja" => Ok(Self::Jinja),
            #[cfg(feature = "python")]
            "dbt" => Ok(Self::Dbt),
            _ => Err(format!(
                "Unknown templater '{}'. Available templaters: {}",
                s,
                Self::available_names().join(", ")
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaceholderStyle {
    Colon,
    ColonNoSpaces,
    NumericColon,
    At,
    Pyformat,
    Dollar,
    FlywayVar,
    QuestionMark,
    NumericDollar,
    Percent,
    Ampersand,
    ApacheCamel,
}

impl PlaceholderStyle {
    pub const fn all() -> &'static [Self] {
        &[
            Self::Colon,
            Self::ColonNoSpaces,
            Self::NumericColon,
            Self::At,
            Self::Pyformat,
            Self::Dollar,
            Self::FlywayVar,
            Self::QuestionMark,
            Self::NumericDollar,
            Self::Percent,
            Self::Ampersand,
            Self::ApacheCamel,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Colon => "colon",
            Self::ColonNoSpaces => "colon_nospaces",
            Self::NumericColon => "numeric_colon",
            Self::At => "at",
            Self::Pyformat => "pyformat",
            Self::Dollar => "dollar",
            Self::FlywayVar => "flyway_var",
            Self::QuestionMark => "question_mark",
            Self::NumericDollar => "numeric_dollar",
            Self::Percent => "percent",
            Self::Ampersand => "ampersand",
            Self::ApacheCamel => "apache_camel",
        }
    }

    pub const fn regex_pattern(self) -> &'static str {
        match self {
            Self::Colon => r"(?<![:\w\\]):(?P<param_name>\w+)(?!:)",
            Self::ColonNoSpaces => r"(?<!:):(?P<param_name>\w+)",
            Self::NumericColon => r"(?<![:\w\\]):(?P<param_name>\d+)",
            Self::At => r"(?<![:\w\\])@(?P<param_name>\w+)",
            Self::Pyformat => r"(?<![:\w\\])%\((?P<param_name>[\w_]+)\)s",
            Self::Dollar => r"(?<![:\w\\])\${?(?P<param_name>[\w_]+)}?",
            Self::FlywayVar => r#"\${(?P<param_name>\w+[:\w_]+)}"#,
            Self::QuestionMark => r"(?<![:\w\\])\?",
            Self::NumericDollar => r"(?<![:\w\\])\${?(?P<param_name>[\d]+)}?",
            Self::Percent => r"(?<![:\w\\])%s",
            Self::Ampersand => r"(?<!&)&{?(?P<param_name>[\w]+)}?",
            Self::ApacheCamel => r":#\$\{(?P<param_name>.+)}",
        }
    }

    pub fn regex(self) -> Regex {
        Regex::new(self.regex_pattern()).unwrap()
    }

    pub fn from_name(s: &str) -> Result<Self, String> {
        match s {
            "colon" => Ok(Self::Colon),
            "colon_nospaces" => Ok(Self::ColonNoSpaces),
            "numeric_colon" => Ok(Self::NumericColon),
            "at" => Ok(Self::At),
            "pyformat" => Ok(Self::Pyformat),
            "dollar" => Ok(Self::Dollar),
            "flyway_var" => Ok(Self::FlywayVar),
            "question_mark" => Ok(Self::QuestionMark),
            "numeric_dollar" => Ok(Self::NumericDollar),
            "percent" => Ok(Self::Percent),
            "ampersand" => Ok(Self::Ampersand),
            "apache_camel" => Ok(Self::ApacheCamel),
            _ => Err(format!("Unknown placeholder style '{s}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaceholderStyle, TemplaterKind};

    #[test]
    fn templater_kind_parses_and_lists_available_names() {
        assert_eq!(TemplaterKind::from_name("raw").unwrap(), TemplaterKind::Raw);
        assert!(TemplaterKind::available_names().contains(&"placeholder"));
    }

    #[test]
    fn placeholder_style_builds_regex() {
        let regex = PlaceholderStyle::QuestionMark.regex();
        assert!(regex.is_match("?").unwrap());
    }
}
