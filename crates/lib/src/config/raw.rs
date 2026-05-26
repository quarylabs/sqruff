use hashbrown::HashMap;

pub use sqruff_lib_core::value::Value;

/// split_comma_separated_string takes a string and splits it on commas and
/// trims and filters out empty strings.
pub fn split_comma_separated_string(raw_str: &str) -> Value {
    let values = raw_str
        .split(',')
        .filter_map(|x| {
            let trimmed = x.trim();
            (!trimmed.is_empty()).then(|| Value::String(trimmed.into()))
        })
        .collect();
    Value::Array(values)
}

pub(crate) fn insert_config_path(ctx: &mut HashMap<String, Value>, path: &[String], value: Value) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };

    if rest.is_empty() {
        ctx.insert(key.to_string(), value);
        return;
    }

    let entry = ctx
        .entry(key.to_string())
        .or_insert_with(|| Value::Map(HashMap::new()));
    if entry.as_map().is_none() {
        *entry = Value::Map(HashMap::new());
    }
    let Some(child) = entry.as_map_mut() else {
        return;
    };

    insert_config_path(child, rest, value);
}

pub(crate) fn nested_combine(config_stack: Vec<HashMap<String, Value>>) -> HashMap<String, Value> {
    let capacity = config_stack.len();
    let mut result = HashMap::with_capacity(capacity);

    for dict in config_stack {
        for (key, value) in dict {
            result.insert(key, value);
        }
    }

    result
}
