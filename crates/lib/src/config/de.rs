use serde::Deserializer;

use super::{NullableSetting, Setting};

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Str(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::Str(s) => s
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            StringOrVec::Vec(v) => v,
        }
    }
}

/// Deserialize a patch setting from either a comma-separated string or a
/// YAML/JSON sequence.
pub(super) fn setting_csv<'de, D>(d: D) -> Result<Setting<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    StringOrVec::deserialize(d).map(|value| Setting::Set(value.into_vec()))
}

/// Deserialize a nullable patch setting from either a comma-separated string,
/// a YAML/JSON sequence, or an explicit null.
pub(super) fn nullable_setting_csv<'de, D>(d: D) -> Result<NullableSetting<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    let opt: Option<StringOrVec> = Option::deserialize(d)?;
    Ok(Setting::Set(opt.map(StringOrVec::into_vec)))
}
