use serde::Deserializer;

/// Deserialize an `Option<Vec<String>>` from either a comma-separated string
/// (`"AL01,AL02"`) or a YAML/JSON sequence (`["AL01", "AL02"]`).
pub(super) fn opt_csv<'de, D>(d: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Str(String),
        Vec(Vec<String>),
    }

    let opt: Option<StringOrVec> = Option::deserialize(d)?;
    Ok(opt.map(|sv| match sv {
        StringOrVec::Str(s) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        StringOrVec::Vec(v) => v,
    }))
}
