use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

pub fn pretty_yaml(text: &str) -> Result<String, String> {
    let value = parse_yaml(text)?;
    serde_yaml::to_string(&value).map_err(|e| e.to_string())
}

pub fn looks_like_yaml(text: &str) -> bool {
    parse_yaml(text).is_ok() && parse_json(text).is_err()
}

fn parse_json(text: &str) -> Result<JsonValue, String> {
    let value: JsonValue =
        serde_json::from_str(text.trim()).map_err(|e| format!("not JSON: {e}"))?;
    if !value.is_object() && !value.is_array() {
        return Err("JSON must be an object or array".into());
    }
    Ok(value)
}

fn parse_yaml(text: &str) -> Result<YamlValue, String> {
    let value: YamlValue =
        serde_yaml::from_str(text.trim()).map_err(|e| format!("not YAML: {e}"))?;
    match value {
        YamlValue::Mapping(_) | YamlValue::Sequence(_) => Ok(value),
        _ => Err("YAML must be a mapping or sequence".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{looks_like_yaml, pretty_yaml};

    #[test]
    fn detects_yaml_not_json() {
        assert!(looks_like_yaml("name: copycraft\nitems:\n  - one\n"));
        assert!(!looks_like_yaml("{\"a\":1}"));
    }

    #[test]
    fn pretty_yaml_keeps_keys() {
        let out = pretty_yaml("name:   copycraft").unwrap();
        assert!(out.contains("name"));
        assert!(out.contains("copycraft"));
    }
}
