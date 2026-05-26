use hashbrown::HashMap;

pub use sqruff_lib_core::value::Value;

pub(crate) type RawConfig = HashMap<String, Value>;

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
    config_stack
        .into_iter()
        .fold(HashMap::new(), deep_merge)
}

fn deep_merge(mut a: HashMap<String, Value>, b: HashMap<String, Value>) -> HashMap<String, Value> {
    for (key, value_b) in b {
        match (a.get(&key), value_b) {
            (Some(Value::Map(map_a)), Value::Map(map_b)) => {
                let combined = deep_merge(map_a.clone(), map_b);
                a.insert(key, Value::Map(combined));
            }
            (_, value) => {
                a.insert(key, value);
            }
        }
    }
    a
}

pub(crate) fn merge_configs(a: HashMap<String, Value>, b: HashMap<String, Value>) -> HashMap<String, Value> {
    deep_merge(a, b)
}
