//! Typed configuration parsing.
//!
//! Configuration reaches the linter as loosely typed [`Value`]s read from the
//! `.sqruff` file. The helpers here turn a section of that raw config into a
//! typed struct, validating every field up front and reporting a user facing
//! error when a value is missing or has the wrong shape. This is the same
//! approach the SQL dialects take with
//! [`dialect_config!`](crate::dialect_config), extended so that a field can be
//! any type implementing [`ConfigField`] rather than just a boolean.

use hashbrown::HashMap;

use crate::value::Value;

/// A section of raw configuration values, keyed by option name.
pub type ConfigMap = HashMap<String, Value>;

/// A value that can be read from a configuration file into a typed field.
pub trait ConfigField: Sized {
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

/// Reads a single field out of a raw configuration section.
///
/// `context` names whatever owns the configuration (for example `Rule ST09`)
/// so that a missing option points at the thing that needed it.
pub fn parse_field<T: ConfigField>(
    context: &str,
    field: &str,
    config: &ConfigMap,
) -> Result<T, String> {
    match config.get(field) {
        Some(value) if !value.is_none() => T::parse(value).ok_or_else(|| {
            format!(
                "Invalid value for {field}: {}. Must be {}",
                describe(value),
                T::expected()
            )
        }),
        _ => Err(format!("{context} expects {} for `{field}`", T::expected())),
    }
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
            /// user facing error for the first invalid option.
            pub fn from_config(config: &$crate::config::ConfigMap) -> Result<Self, String> {
                Ok(Self {
                    $($field: $crate::config::parse_field::<$ty>(
                        $context,
                        stringify!($field),
                        config,
                    )?,)*
                })
            }

            /// The options this configuration accepts, as
            /// `(name, description, default)`.
            pub fn config_options() -> Vec<(&'static str, &'static str, String)> {
                let defaults = Self::default();
                vec![
                    $((
                        stringify!($field),
                        $desc,
                        $crate::config::ConfigField::render(&defaults.$field),
                    ),)*
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
                strict: true
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
    fn rejects_a_missing_value() {
        let err = TestConfig::from_config(&config(&[("strict", Value::Bool(false))])).unwrap_err();

        assert_eq!(err, "Rule XX01 expects one of [sweet, sour] for `flavour`");
    }

    #[test]
    fn reports_defaults_for_documentation() {
        assert_eq!(
            TestConfig::config_options(),
            vec![
                ("flavour", "Which flavour to enforce.", "sweet".to_string()),
                ("strict", "Whether to be strict.", "false".to_string()),
            ]
        );
    }
}
