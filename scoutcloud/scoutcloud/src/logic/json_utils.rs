use json_dotpath::DotPaths;

pub fn update_json_by_path(
    json: &mut serde_json::Value,
    path: &str,
    new_value: serde_json::Value,
) -> Result<(), json_dotpath::Error> {
    json.dot_set(path, new_value)?;
    Ok(())
}

pub fn merge(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => *a = b.clone(),
    }
}

pub fn filter_null_values(json: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = json {
        for key in map
            .iter()
            .filter(|(_, v)| v.is_null())
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>()
        {
            map.remove(&key);
        }
    }
}
