use std::collections::HashMap as StdHashMap;
use std::fmt;
use std::str::FromStr;

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use serde::{Deserialize, Deserializer};

use super::{ConfigError, NullableSetting, Setting};

#[derive(Debug)]
struct Error(String);

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self(msg.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

pub(crate) fn deserialize_section<T>(
    section_name: &str,
    values: &StdHashMap<String, Option<String>>,
) -> std::result::Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    T::deserialize(SectionDeserializer {
        values,
        section_name,
    })
    .map_err(|err| ConfigError::InvalidSection {
        section: section_name.to_string(),
        reason: err.to_string(),
    })
}

pub(crate) fn deserialize_toml_table<T>(
    section_name: &str,
    table: &toml::value::Table,
) -> std::result::Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    toml::Value::Table(table.clone())
        .try_into()
        .map_err(|err| ConfigError::InvalidSection {
            section: section_name.to_string(),
            reason: err.to_string(),
        })
}

struct SectionDeserializer<'a> {
    values: &'a StdHashMap<String, Option<String>>,
    section_name: &'a str,
}

impl<'de> Deserializer<'de> for SectionDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(SectionMapAccess {
            iter: self.values.iter(),
            value: None,
            section_name: self.section_name,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
        enum identifier ignored_any
    }
}

struct SectionMapAccess<'a> {
    iter: std::collections::hash_map::Iter<'a, String, Option<String>>,
    value: Option<&'a Option<String>>,
    section_name: &'a str,
}

impl<'de> MapAccess<'de> for SectionMapAccess<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let Some((key, value)) = self.iter.next() else {
            return Ok(None);
        };
        self.value = Some(value);
        seed.deserialize(key.as_str().into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let value = self
            .value
            .take()
            .ok_or_else(|| Error(format!("missing value in section '{}'", self.section_name)))?;
        seed.deserialize(ScalarDeserializer {
            value: value.as_deref(),
        })
    }
}

#[derive(Clone, Copy)]
struct ScalarDeserializer<'a> {
    value: Option<&'a str>,
}

impl<'a> ScalarDeserializer<'a> {
    fn raw(self) -> &'a str {
        self.value.unwrap_or_default()
    }

    fn trimmed(self) -> String {
        self.raw().trim().to_string()
    }

    fn is_none(self) -> bool {
        let raw = self.raw().trim();
        raw.is_empty() || raw.eq_ignore_ascii_case("none")
    }
}

impl<'de> Deserializer<'de> for ScalarDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw();
        let trimmed = raw.trim();

        if self.is_none() {
            visitor.visit_unit()
        } else if trimmed.eq_ignore_ascii_case("true") {
            visitor.visit_bool(true)
        } else if trimmed.eq_ignore_ascii_case("false") {
            visitor.visit_bool(false)
        } else if let Ok(value) = trimmed.parse::<i64>() {
            visitor.visit_i64(value)
        } else if let Ok(value) = trimmed.parse::<f64>() {
            visitor.visit_f64(value)
        } else {
            visitor.visit_borrowed_str(raw)
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        if raw.eq_ignore_ascii_case("true") {
            visitor.visit_bool(true)
        } else if raw.eq_ignore_ascii_case("false") {
            visitor.visit_bool(false)
        } else {
            Err(Error(format!("invalid bool '{raw}'")))
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(IntVisitor::new(visitor))
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(IntVisitor::new(visitor))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        raw.parse::<i32>()
            .map_err(|_| Error(format!("invalid integer '{raw}'")))
            .and_then(|value| visitor.visit_i32(value))
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        raw.parse::<i64>()
            .map_err(|_| Error(format!("invalid integer '{raw}'")))
            .and_then(|value| visitor.visit_i64(value))
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(UintVisitor::new(visitor))
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(UintVisitor::new(visitor))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(UintVisitor::new(visitor))
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        raw.parse::<u64>()
            .map_err(|_| Error(format!("invalid integer '{raw}'")))
            .and_then(|value| visitor.visit_u64(value))
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        raw.parse::<f32>()
            .map_err(|_| Error(format!("invalid float '{raw}'")))
            .and_then(|value| visitor.visit_f32(value))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw().trim();
        raw.parse::<f64>()
            .map_err(|_| Error(format!("invalid float '{raw}'")))
            .and_then(|value| visitor.visit_f64(value))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.raw())
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.raw().to_string())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.is_none() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.is_none() {
            visitor.visit_unit()
        } else {
            Err(Error(format!("expected None, found '{}'", self.raw())))
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let values = self
            .raw()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        visitor.visit_seq(CsvSeqAccess {
            values: values.into_iter(),
        })
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self.trimmed().into_deserializer())
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    serde::forward_to_deserialize_any! {
        char bytes byte_buf unit_struct newtype_struct tuple tuple_struct
        map struct ignored_any
    }
}

struct IntVisitor<V> {
    visitor: V,
}

impl<V> IntVisitor<V> {
    fn new(visitor: V) -> Self {
        Self { visitor }
    }
}

impl<'de, V> Visitor<'de> for IntVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.visitor.expecting(formatter)
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visitor.visit_i64(value)
    }
}

struct UintVisitor<V> {
    visitor: V,
}

impl<V> UintVisitor<V> {
    fn new(visitor: V) -> Self {
        Self { visitor }
    }
}

impl<'de, V> Visitor<'de> for UintVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.visitor.expecting(formatter)
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visitor.visit_u64(value)
    }
}

struct CsvSeqAccess<'a> {
    values: std::vec::IntoIter<&'a str>,
}

impl<'de> SeqAccess<'de> for CsvSeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let Some(value) = self.values.next() else {
            return Ok(None);
        };
        seed.deserialize(ScalarDeserializer { value: Some(value) })
            .map(Some)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum StringOrVec {
    Str(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    pub(crate) fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::Str(s) => s
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"))
                .collect(),
            StringOrVec::Vec(v) => v.into_iter().filter(|s| !is_none_value(s)).collect(),
        }
    }

    pub(crate) fn into_optional_vec(self) -> Option<Vec<String>> {
        match self {
            StringOrVec::Str(s) => optional_csv(&s),
            StringOrVec::Vec(v) => Some(v.into_iter().filter(|s| !is_none_value(s)).collect()),
        }
    }
}

pub(crate) fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!is_none_value(trimmed)).then(|| value.to_string())
}

pub(crate) fn csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn optional_csv(value: &str) -> Option<Vec<String>> {
    optional_string(value).map(|value| csv(&value))
}

fn is_none_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none")
}

pub(super) fn setting_from_str<'de, D, T>(d: D) -> std::result::Result<Setting<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let value = String::deserialize(d)?;
    value
        .parse()
        .map(Setting::Set)
        .map_err(|err: T::Err| de::Error::custom(err.to_string()))
}

pub(super) fn setting_optional_string<'de, D>(
    d: D,
) -> std::result::Result<Setting<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(d)?
        .map(Setting::Set)
        .unwrap_or(Setting::Unset))
}

pub(super) fn nullable_setting_from_str<'de, D, T>(
    d: D,
) -> std::result::Result<NullableSetting<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let value = Option::<String>::deserialize(d)?;
    value
        .map(|value| {
            value
                .parse()
                .map_err(|err: T::Err| de::Error::custom(err.to_string()))
        })
        .transpose()
        .map(Setting::Set)
}

/// Deserialize a patch setting from either a comma-separated string or a
/// YAML/JSON sequence.
pub(super) fn setting_csv<'de, D>(d: D) -> std::result::Result<Setting<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<StringOrVec>::deserialize(d)
        .map(|value| Setting::Set(value.map(StringOrVec::into_vec).unwrap_or_default()))
}

pub(super) fn nullable_setting_csv<'de, D>(
    d: D,
) -> std::result::Result<NullableSetting<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<StringOrVec>::deserialize(d)
        .map(|value| Setting::Set(value.and_then(StringOrVec::into_optional_vec)))
}

pub(super) fn nonnegative_usize_setting<'de, D>(
    d: D,
) -> std::result::Result<Setting<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(d).map(|value| Setting::Set(value.max(0) as usize))
}

pub(super) fn saturated_u8_setting<'de, D>(d: D) -> std::result::Result<Setting<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(d).map(|value| Setting::Set(value.clamp(0, i64::from(u8::MAX)) as u8))
}
