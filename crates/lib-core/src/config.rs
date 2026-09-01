//! Typed configuration parsing.
//!
//! Configuration reaches the linter as loosely typed [`Value`]s read from the
//! `.sqruff` file. The helpers here turn a section of that raw config into a
//! typed struct, validating every field up front and reporting a user facing
//! error when a value has the wrong shape. Missing values use the typed
//! declaration's default, making that declaration the source of truth. This is the same
//! approach the SQL dialects take with
//! [`dialect_config!`](crate::dialect_config), extended so that a field can be
//! any type implementing [`ConfigField`] rather than just a boolean.

use hashbrown::HashMap;

use crate::value::Value;

/// A section of raw configuration values, keyed by option name.
pub type ConfigMap = HashMap<String, Value>;

/// The shape of a typed configuration option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKind {
    Boolean,
    Integer,
    String,
    StringList,
    Enum(&'static [&'static str]),
}

impl ConfigKind {
    pub fn description(self) -> String {
        match self {
            Self::Boolean => "boolean".into(),
            Self::Integer => "integer".into(),
            Self::String => "string".into(),
            Self::StringList => "list of strings".into(),
            Self::Enum(values) => format!("one of: {}", values.join(", ")),
        }
    }
}

/// Metadata for one typed configuration option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOption {
    pub name: &'static str,
    pub description: &'static str,
    pub default: String,
    pub kind: ConfigKind,
}

/// A value that can be read from a configuration file into a typed field.
pub trait ConfigField: Sized {
    /// The machine-readable shape of this field.
    fn kind() -> ConfigKind;

    /// Describes what this field accepts, e.g. `a boolean` or
    /// `one of [earlier, later]`. Used in error messages.
    fn expected() -> String;

    /// Parses the field from its raw configuration value, returning [`None`]
    /// when the value is not valid for this type.
    fn parse(value: &Value) -> Option<Self>;

    /// Renders the field the way it is written in a configuration file.
    fn render(&self) -> String;
}

impl ConfigField for bool {
    fn kind() -> ConfigKind {
        ConfigKind::Boolean
    }

    fn expected() -> String {
        "a boolean".into()
    }

    fn parse(value: &Value) -> Option<Self> {
        value.as_bool()
    }

    fn render(&self) -> String {
        self.to_string()
    }
}

impl ConfigField for i32 {
    fn kind() -> ConfigKind {
        ConfigKind::Integer
    }

    fn expected() -> String {
        "an integer".into()
    }

    fn parse(value: &Value) -> Option<Self> {
        value.as_int()
    }

    fn render(&self) -> String {
        self.to_string()
    }
}

impl ConfigField for String {
    fn kind() -> ConfigKind {
        ConfigKind::String
    }

    fn expected() -> String {
        "a string".into()
    }

    fn parse(value: &Value) -> Option<Self> {
        value.as_string().map(ToOwned::to_owned)
    }

    fn render(&self) -> String {
        self.clone()
    }
}

impl ConfigField for Vec<String> {
    fn kind() -> ConfigKind {
        ConfigKind::StringList
    }

    fn expected() -> String {
        "a list of strings".into()
    }

    fn parse(value: &Value) -> Option<Self> {
        value
            .as_array()?
            .iter()
            .map(|item| item.as_string().map(ToOwned::to_owned))
            .collect()
    }

    fn render(&self) -> String {
        self.join(",")
    }
}

/// Reads a single field out of a raw configuration section.
pub fn parse_field<T: ConfigField>(field: &str, config: &ConfigMap) -> Result<Option<T>, String> {
    match config.get(field) {
        Some(value) if !value.is_none() => T::parse(value).map(Some).ok_or_else(|| {
            format!(
                "Invalid value for {field}: {}. Must be {}",
                describe(value),
                T::expected()
            )
        }),
        _ => Ok(None),
    }
}

/// Rejects misspelled or otherwise unsupported keys.
pub fn validate_keys(
    context: &str,
    config: &ConfigMap,
    expected: &[&'static str],
) -> Result<(), String> {
    if let Some(key) = config.keys().find(|key| !expected.contains(&key.as_str())) {
        return Err(format!(
            "Unknown configuration option `{key}` for {context}. Expected one of [{}]",
            expected.join(", ")
        ));
    }
    Ok(())
}

/// Renders a raw config value for inclusion in an error message.
fn describe(value: &Value) -> String {
    match value {
        Value::Int(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::String(v) => v.to_string(),
        Value::Map(_) => "a section".into(),
        Value::Array(_) => "a list".into(),
        Value::None => "none".into(),
    }
}

/// Defines a configuration option that accepts one of a fixed set of strings.
///
/// The generated enum knows how each variant is spelled in the config file and
/// implements [`ConfigField`], so it can be used as a field type in
/// [`typed_config!`](crate::typed_config).
///
/// # Usage
///
/// ```ignore
/// sqruff_lib_core::config_enum!(
///     /// Which table a join condition should reference first.
///     PreferredFirstTableInJoinClause {
///         /// The table referenced earlier in the statement.
///         Earlier = "earlier",
///         /// The table referenced later in the statement.
///         Later = "later",
///     }
/// );
/// ```
#[macro_export]
macro_rules! config_enum {
    (
        $(#[doc = $enum_doc:expr])*
        $name:ident {
            $(
                $(#[doc = $variant_doc:expr])*
                $variant:ident = $raw:expr
            ),* $(,)?
        }
    ) => {
        $(#[doc = $enum_doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[doc = $variant_doc])* $variant,)*
        }

        impl $name {
            /// Every accepted value, in declaration order.
            pub const VARIANTS: &'static [&'static str] = &[$($raw,)*];

            /// The value as it is written in the config file.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $raw,)*
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl $crate::config::ConfigField for $name {
            fn kind() -> $crate::config::ConfigKind {
                $crate::config::ConfigKind::Enum(Self::VARIANTS)
            }

            fn expected() -> String {
                format!("one of [{}]", Self::VARIANTS.join(", "))
            }

            fn parse(value: &$crate::value::Value) -> Option<Self> {
                match value.as_string()? {
                    $($raw => Some(Self::$variant),)*
                    _ => None,
                }
            }

            fn render(&self) -> String {
                self.as_str().to_string()
            }
        }
    };
}

/// Generates a typed configuration struct, its defaults, a validating parser
/// and the option metadata used to generate documentation.
///
/// # Usage
///
/// ```ignore
/// sqruff_lib_core::typed_config!(
///     /// Configuration for `RuleST09`.
///     RuleST09Config, context = "Rule ST09", {
///         /// Which table to list first in a join condition.
///         preferred_first_table_in_join_clause: PreferredFirstTableInJoinClause
///             = PreferredFirstTableInJoinClause::Earlier,
///             "Which table a join condition should reference first.",
///     }
/// );
/// ```
#[macro_export]
macro_rules! typed_config {
    (
        $(#[doc = $struct_doc:expr])*
        $name:ident, context = $context:expr, {
            $(
                $(#[doc = $field_doc:expr])*
                $field:ident : $ty:ty = $default:expr, $desc:expr
            ),* $(,)?
        }
    ) => {
        $(#[doc = $struct_doc])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            $($(#[doc = $field_doc])* pub $field: $ty,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }

        impl $name {
            /// Parses and validates the configuration section, returning a
            /// user facing error for the first invalid option. Missing fields
            /// retain their declared defaults.
            pub fn from_config(config: &$crate::config::ConfigMap) -> Result<Self, String> {
                $crate::config::validate_keys(
                    $context,
                    config,
                    &[$(stringify!($field),)*],
                )?;
                let mut parsed = Self::default();
                $(
                    if let Some(value) = $crate::config::parse_field::<$ty>(
                        stringify!($field),
                        config,
                    )? {
                        parsed.$field = value;
                    }
                )*
                Ok(parsed)
            }

            /// The options this configuration accepts.
            pub fn config_options() -> Vec<$crate::config::ConfigOption> {
                let defaults = Self::default();
                vec![
                    $($crate::config::ConfigOption {
                        name: stringify!($field),
                        description: $desc,
                        default: $crate::config::ConfigField::render(&defaults.$field),
                        kind: <$ty as $crate::config::ConfigField>::kind(),
                    },)*
                ]
            }
        }
    };
}

#[cfg(test)]
#[allow(
    unreachable_pub,
    reason = "the macros generate public items for public modules"
)]
mod tests {
    use super::*;

    config_enum!(
        /// Test enum.
        Flavour {
            /// Sweet.
            Sweet = "sweet",
            /// Sour.
            Sour = "sour",
        }
    );

    typed_config!(
        /// Test config.
        TestConfig, context = "Rule XX01", {
            /// The flavour.
            flavour: Flavour = Flavour::Sweet, "Which flavour to enforce.",
            /// Whether to be strict.
            strict: bool = false, "Whether to be strict.",
            /// Maximum attempts.
            attempts: i32 = 3, "How many times to try.",
            /// Labels to include.
            labels: Vec<String> = vec!["stable".into()], "Labels to include.",
        }
    );

    fn config(values: &[(&str, Value)]) -> ConfigMap {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn parses_a_valid_section() {
        let parsed = TestConfig::from_config(&config(&[
            ("flavour", Value::String("sour".into())),
            ("strict", Value::Bool(true)),
        ]))
        .unwrap();

        assert_eq!(
            parsed,
            TestConfig {
                flavour: Flavour::Sour,
                strict: true,
                attempts: 3,
                labels: vec!["stable".into()],
            }
        );
    }

    #[test]
    fn rejects_a_value_outside_the_enum() {
        let err = TestConfig::from_config(&config(&[
            ("flavour", Value::String("umami".into())),
            ("strict", Value::Bool(true)),
        ]))
        .unwrap_err();

        assert_eq!(
            err,
            "Invalid value for flavour: umami. Must be one of [sweet, sour]"
        );
    }

    #[test]
    fn rejects_a_value_of_the_wrong_type() {
        let err = TestConfig::from_config(&config(&[
            ("flavour", Value::String("sour".into())),
            ("strict", Value::Int(1)),
        ]))
        .unwrap_err();

        assert_eq!(err, "Invalid value for strict: 1. Must be a boolean");
    }

    #[test]
    fn uses_defaults_for_missing_values() {
        let parsed = TestConfig::from_config(&config(&[("strict", Value::Bool(true))])).unwrap();

        assert_eq!(parsed.flavour, Flavour::Sweet);
        assert!(parsed.strict);
        assert_eq!(parsed.attempts, 3);
        assert_eq!(parsed.labels, vec!["stable"]);
    }

    #[test]
    fn parses_integer_and_list_fields() {
        let parsed = TestConfig::from_config(&config(&[
            ("attempts", Value::Int(5)),
            ("labels", Value::String("fast,safe".into())),
        ]))
        .unwrap();

        assert_eq!(parsed.attempts, 5);
        assert_eq!(parsed.labels, vec!["fast", "safe"]);
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = TestConfig::from_config(&config(&[("flavor", Value::String("sour".into()))]))
            .unwrap_err();

        assert_eq!(
            err,
            "Unknown configuration option `flavor` for Rule XX01. Expected one of [flavour, \
             strict, attempts, labels]"
        );
    }

    #[test]
    fn reports_defaults_for_documentation() {
        assert_eq!(
            TestConfig::config_options(),
            vec![
                ConfigOption {
                    name: "flavour",
                    description: "Which flavour to enforce.",
                    default: "sweet".into(),
                    kind: ConfigKind::Enum(&["sweet", "sour"]),
                },
                ConfigOption {
                    name: "strict",
                    description: "Whether to be strict.",
                    default: "false".into(),
                    kind: ConfigKind::Boolean,
                },
                ConfigOption {
                    name: "attempts",
                    description: "How many times to try.",
                    default: "3".into(),
                    kind: ConfigKind::Integer,
                },
                ConfigOption {
                    name: "labels",
                    description: "Labels to include.",
                    default: "stable".into(),
                    kind: ConfigKind::StringList,
                },
            ]
        );
    }
}
