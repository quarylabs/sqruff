//! Typed and validated configuration for lint rules.
//!
//! Rules declare their configuration once with [`rule_config!`](crate::rule_config),
//! mirroring the way dialects declare theirs with `dialect_config!`. That single
//! declaration drives:
//!
//! * parsing the raw `[sqruff:rules:*]` values into Rust types,
//! * validating them, so a mistyped option is reported with the option name and
//!   the accepted values instead of panicking or being silently ignored, and
//! * documenting them in `docs/reference/rules.md`.

use hashbrown::HashMap;
use regex::Regex;

use crate::core::config::Value;

/// A single documented configuration option of a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConfigOption {
    /// The key used in the rule's `[sqruff:rules:<name>]` section.
    pub name: &'static str,
    /// What the option does.
    pub description: &'static str,
    /// The kind of value expected, e.g. `boolean` or `integer`.
    pub type_name: &'static str,
    /// The value used when the option is not set.
    pub default: String,
    /// The accepted values, when the option is constrained to a fixed set.
    pub allowed_values: Vec<&'static str>,
}

/// The typed configuration of a rule.
///
/// Implemented by [`rule_config!`](crate::rule_config); rules should not
/// implement this by hand.
pub trait RuleConfig: Default + Sized {
    /// Parse and validate a rule's config section.
    fn from_config(config: &HashMap<String, Value>) -> Result<Self, String>;

    /// The documented options of this configuration.
    fn config_options() -> Vec<RuleConfigOption>;
}

/// A value a rule configuration option can hold.
pub trait RuleConfigValue: Sized {
    /// The kind of value expected, e.g. `boolean` or `integer`.
    fn type_name() -> &'static str;

    /// The accepted values, when constrained to a fixed set.
    fn allowed_values() -> Vec<&'static str> {
        Vec::new()
    }

    /// Parse and validate a raw config value.
    fn from_config_value(value: &Value) -> Result<Self, String>;

    /// How this value is spelled in configuration, for documentation.
    fn render_default(&self) -> String;
}

/// Read a single option out of a rule's config section, falling back to
/// `default` when it is absent.
pub fn parse_option<T: RuleConfigValue>(
    config: &HashMap<String, Value>,
    name: &'static str,
    default: T,
) -> Result<T, String> {
    let Some(value) = config.get(name) else {
        return Ok(default);
    };

    T::from_config_value(value).map_err(|err| format!("Invalid value for `{name}`: {err}"))
}

/// The error used when a value is not one of a fixed set.
pub fn one_of_error(got: &str, allowed: &[&str]) -> String {
    format!("expected one of [{}], got `{got}`", allowed.join(", "))
}

/// Parse a value into an enum declared with
/// [`rule_config_enum!`](crate::rule_config_enum).
pub fn parse_enum_value<T>(value: &Value) -> Result<T, String>
where
    T: std::str::FromStr<Err = String>,
{
    match value {
        // The config loader turns a literal `none` into `Value::None`, so enums
        // that accept `none` have to recognise it here.
        Value::None => "none".parse(),
        Value::String(raw) => raw.trim().parse(),
        Value::Bool(raw) => raw.to_string().parse(),
        Value::Int(raw) => raw.to_string().parse(),
        other => Err(format!("expected a string, got {}", describe(other))),
    }
}

/// Describe a raw value for use in an error message.
pub fn describe(value: &Value) -> String {
    match value {
        Value::Int(value) => format!("`{value}`"),
        Value::Bool(value) => format!("`{value}`"),
        Value::Float(value) => format!("`{value}`"),
        Value::String(value) => format!("`{value}`"),
        Value::Map(_) => "a section".to_string(),
        Value::Array(values) => format!("a list of {} values", values.len()),
        Value::None => "`none`".to_string(),
    }
}

/// Split a raw value into a list of strings, accepting both a comma separated
/// string (INI) and a native list (TOML).
fn value_to_string_list(value: &Value) -> Result<Vec<String>, String> {
    let raw = match value {
        Value::None => return Ok(Vec::new()),
        Value::String(raw) => raw
            .split(',')
            .map(|item| item.trim().to_string())
            .collect::<Vec<_>>(),
        // A bare `true`/`false`/number is parsed by the config loader before the
        // rule sees it, so an unquoted word list entry such as `ignore_words =
        // true` arrives already converted. Take it back as the word it was.
        Value::Bool(value) => vec![value.to_string()],
        Value::Int(value) => vec![value.to_string()],
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(raw) => Ok(raw.trim().to_string()),
                Value::Int(value) => Ok(value.to_string()),
                Value::Bool(value) => Ok(value.to_string()),
                other => Err(format!(
                    "expected a list of strings, got {} in the list",
                    describe(other)
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        other => {
            return Err(format!(
                "expected a comma separated list of strings, got {}",
                describe(other)
            ));
        }
    };

    Ok(raw.into_iter().filter(|item| !item.is_empty()).collect())
}

impl RuleConfigValue for bool {
    fn type_name() -> &'static str {
        "boolean"
    }

    fn allowed_values() -> Vec<&'static str> {
        vec!["true", "false"]
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        match value {
            Value::Bool(value) => Ok(*value),
            Value::String(raw) if raw.trim().eq_ignore_ascii_case("true") => Ok(true),
            Value::String(raw) if raw.trim().eq_ignore_ascii_case("false") => Ok(false),
            other => Err(format!(
                "expected a boolean (`true` or `false`), got {}",
                describe(other)
            )),
        }
    }

    fn render_default(&self) -> String {
        self.to_string()
    }
}

impl RuleConfigValue for usize {
    fn type_name() -> &'static str {
        "integer"
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        let parsed = match value {
            Value::Int(value) => (*value).try_into().ok(),
            Value::String(raw) => raw.trim().parse().ok(),
            other => {
                return Err(format!(
                    "expected a non-negative integer, got {}",
                    describe(other)
                ));
            }
        };

        parsed.ok_or_else(|| format!("expected a non-negative integer, got {}", describe(value)))
    }

    fn render_default(&self) -> String {
        self.to_string()
    }
}

impl RuleConfigValue for String {
    fn type_name() -> &'static str {
        "string"
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        match value {
            Value::String(raw) => Ok(raw.to_string()),
            Value::Int(value) => Ok(value.to_string()),
            Value::Bool(value) => Ok(value.to_string()),
            other => Err(format!("expected a string, got {}", describe(other))),
        }
    }

    fn render_default(&self) -> String {
        if self.is_empty() {
            "none".to_string()
        } else {
            self.clone()
        }
    }
}

impl<T: RuleConfigValue> RuleConfigValue for Option<T> {
    fn type_name() -> &'static str {
        T::type_name()
    }

    fn allowed_values() -> Vec<&'static str> {
        let mut allowed = T::allowed_values();
        if !allowed.is_empty() {
            allowed.push("none");
        }
        allowed
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        if value.is_none() {
            return Ok(None);
        }
        T::from_config_value(value).map(Some)
    }

    fn render_default(&self) -> String {
        match self {
            Some(value) => value.render_default(),
            None => "none".to_string(),
        }
    }
}

impl RuleConfigValue for Vec<String> {
    fn type_name() -> &'static str {
        "comma separated list of strings"
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        value_to_string_list(value)
    }

    fn render_default(&self) -> String {
        if self.is_empty() {
            "none".to_string()
        } else {
            self.join(",")
        }
    }
}

impl RuleConfigValue for Vec<Regex> {
    fn type_name() -> &'static str {
        "comma separated list of regular expressions"
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        value_to_string_list(value)?
            .into_iter()
            .map(|pattern| {
                Regex::new(&pattern)
                    .map_err(|err| format!("`{pattern}` is not a valid regular expression: {err}"))
            })
            .collect()
    }

    fn render_default(&self) -> String {
        if self.is_empty() {
            "none".to_string()
        } else {
            self.iter().map(Regex::as_str).collect::<Vec<_>>().join(",")
        }
    }
}

/// A comma separated list of words, normalised to lowercase at load time so
/// that rules compare against them case-insensitively.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IgnoreWords(Vec<String>);

impl IgnoreWords {
    /// Build a list from any iterator of words, lowercasing each one.
    pub fn new(words: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self(
            words
                .into_iter()
                .map(|word| word.as_ref().to_lowercase())
                .collect(),
        )
    }

    /// Whether `word` is in the list, compared case-insensitively.
    pub fn matches(&self, word: &str) -> bool {
        !self.0.is_empty() && self.0.contains(&word.to_lowercase())
    }
}

impl std::ops::Deref for IgnoreWords {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RuleConfigValue for IgnoreWords {
    fn type_name() -> &'static str {
        "comma separated list of strings"
    }

    fn from_config_value(value: &Value) -> Result<Self, String> {
        value_to_string_list(value).map(IgnoreWords::new)
    }

    fn render_default(&self) -> String {
        if self.0.is_empty() {
            "none".to_string()
        } else {
            self.0.join(",")
        }
    }
}

/// Declare a string-valued configuration enum.
///
/// The declared spellings are the values accepted in configuration, and are
/// also what gets reported when a value is rejected.
///
/// ```ignore
/// crate::rule_config_enum! {
///     /// Which aliasing style to enforce.
///     #[derive(Default)]
///     pub enum Aliasing {
///         /// Require the `AS` keyword.
///         #[default]
///         Explicit => "explicit",
///         /// Forbid the `AS` keyword.
///         Implicit => "implicit",
///     }
/// }
/// ```
#[macro_export]
macro_rules! rule_config_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $repr:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            /// Every value accepted in configuration, in declaration order.
            pub const VARIANTS: &'static [&'static str] = &[$($repr,)+];

            /// How this variant is spelled in configuration.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $repr,)+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($repr => Ok(Self::$variant),)+
                    other => Err($crate::core::rules::config::one_of_error(
                        other,
                        Self::VARIANTS,
                    )),
                }
            }
        }

        impl $crate::core::rules::config::RuleConfigValue for $name {
            fn type_name() -> &'static str {
                "string"
            }

            fn allowed_values() -> Vec<&'static str> {
                Self::VARIANTS.to_vec()
            }

            fn from_config_value(
                value: &$crate::core::config::Value,
            ) -> Result<Self, String> {
                $crate::core::rules::config::parse_enum_value::<Self>(value)
            }

            fn render_default(&self) -> String {
                self.as_str().to_owned()
            }
        }
    };
}

/// Declare a rule's typed configuration.
///
/// Every field needs a doc comment (it becomes the option's documentation) and
/// a default (used when the option is absent from the configuration).
///
/// ```ignore
/// crate::rule_config! {
///     /// Configuration for `layout.long_lines` (LT05).
///     RuleLT05Config {
///         /// Should long comment lines be ignored?
///         ignore_comment_lines: bool = false,
///         /// Should comment clauses be ignored?
///         ignore_comment_clauses: bool = false,
///     }
/// }
/// ```
#[macro_export]
macro_rules! rule_config {
    (
        $(#[$struct_meta:meta])*
        $name:ident {
            $(
                $(#[doc = $doc:expr])+
                $field:ident : $ty:ty = $default:expr
            ),* $(,)?
        }
    ) => {
        $(#[$struct_meta])*
        #[derive(Debug, Clone)]
        pub struct $name {
            $($(#[doc = $doc])+ pub $field: $ty,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }

        impl $crate::core::rules::config::RuleConfig for $name {
            fn from_config(
                config: &hashbrown::HashMap<String, $crate::core::config::Value>,
            ) -> Result<Self, String> {
                Ok(Self {
                    $($field: $crate::core::rules::config::parse_option(
                        config,
                        stringify!($field),
                        $default,
                    )?,)*
                })
            }

            fn config_options() -> Vec<$crate::core::rules::config::RuleConfigOption> {
                vec![
                    $($crate::core::rules::config::RuleConfigOption {
                        name: stringify!($field),
                        description: concat!($($doc),+).trim_ascii(),
                        type_name: <$ty as
                            $crate::core::rules::config::RuleConfigValue>::type_name(),
                        default: {
                            let default: $ty = $default;
                            $crate::core::rules::config::RuleConfigValue::render_default(&default)
                        },
                        allowed_values: <$ty as
                            $crate::core::rules::config::RuleConfigValue>::allowed_values(),
                    },)*
                ]
            }
        }
    };
}

#[cfg(test)]
// The macros deliberately generate `pub` items; inside a test module nothing
// is reachable from outside the crate.
#[allow(unreachable_pub)]
mod tests {
    use super::*;

    crate::rule_config_enum! {
        /// A test policy.
        #[derive(Default)]
        pub enum TestPolicy {
            /// Do everything.
            #[default]
            All => "all",
            /// Do nothing.
            None => "none",
        }
    }

    crate::rule_config! {
        /// A test configuration.
        TestConfig {
            /// A flag.
            flag: bool = false,
            /// A count.
            count: usize = 3,
            /// A policy.
            policy: TestPolicy = TestPolicy::All,
            /// An optional bound.
            bound: Option<usize> = None,
            /// Words to ignore.
            ignore_words: IgnoreWords = IgnoreWords::default(),
            /// Patterns to ignore.
            ignore_words_regex: Vec<Regex> = Vec::new(),
        }
    }

    fn config(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn missing_options_fall_back_to_defaults() {
        let parsed = TestConfig::from_config(&HashMap::new()).unwrap();

        assert!(!parsed.flag);
        assert_eq!(parsed.count, 3);
        assert_eq!(parsed.policy, TestPolicy::All);
        assert_eq!(parsed.bound, None);
        assert!(parsed.ignore_words.is_empty());
        assert!(parsed.ignore_words_regex.is_empty());
    }

    #[test]
    fn values_are_parsed_into_their_declared_types() {
        let parsed = TestConfig::from_config(&config(&[
            ("flag", Value::Bool(true)),
            ("count", Value::Int(7)),
            ("policy", Value::None),
            ("bound", Value::Int(2)),
            ("ignore_words", Value::String("Foo, BAR".into())),
            ("ignore_words_regex", Value::String("^a.*$".into())),
        ]))
        .unwrap();

        assert!(parsed.flag);
        assert_eq!(parsed.count, 7);
        // A literal `none` reaches the rule as `Value::None`, and has to map
        // onto the `none` variant rather than the default.
        assert_eq!(parsed.policy, TestPolicy::None);
        assert_eq!(parsed.bound, Some(2));
        assert_eq!(parsed.ignore_words.len(), 2);
        assert!(parsed.ignore_words.matches("fOo"));
        assert!(parsed.ignore_words_regex[0].is_match("abc"));
    }

    #[test]
    fn invalid_enum_value_lists_the_accepted_values() {
        let err = TestConfig::from_config(&config(&[("policy", Value::String("nope".into()))]))
            .unwrap_err();

        assert_eq!(
            err,
            "Invalid value for `policy`: expected one of [all, none], got `nope`"
        );
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        let err =
            TestConfig::from_config(&config(&[("flag", Value::String("yes".into()))])).unwrap_err();

        assert_eq!(
            err,
            "Invalid value for `flag`: expected a boolean (`true` or `false`), got `yes`"
        );
    }

    #[test]
    fn negative_integer_is_rejected() {
        let err = TestConfig::from_config(&config(&[("count", Value::Int(-1))])).unwrap_err();

        assert_eq!(
            err,
            "Invalid value for `count`: expected a non-negative integer, got `-1`"
        );
    }

    #[test]
    fn invalid_regex_is_rejected() {
        let err = TestConfig::from_config(&config(&[(
            "ignore_words_regex",
            Value::String("[".into()),
        )]))
        .unwrap_err();

        assert!(
            err.starts_with(
                "Invalid value for `ignore_words_regex`: `[` is not a valid regular expression:"
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn config_options_describe_every_field() {
        let options = TestConfig::config_options();

        assert_eq!(
            options.iter().map(|it| it.name).collect::<Vec<_>>(),
            [
                "flag",
                "count",
                "policy",
                "bound",
                "ignore_words",
                "ignore_words_regex"
            ]
        );

        let policy = &options[2];
        assert_eq!(policy.description, "A policy.");
        assert_eq!(policy.default, "all");
        assert_eq!(policy.allowed_values, ["all", "none"]);

        let bound = &options[3];
        assert_eq!(bound.default, "none");
        assert_eq!(bound.type_name, "integer");
    }
}
