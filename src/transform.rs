use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    PrettyJson,
    MinifyJson,
    JsonToYaml,
    YamlToJson,
}

impl Transform {
    pub fn id(self) -> &'static str {
        match self {
            Self::PrettyJson => "xform_pretty_json",
            Self::MinifyJson => "xform_minify_json",
            Self::JsonToYaml => "xform_json_yaml",
            Self::YamlToJson => "xform_yaml_json",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "xform_pretty_json" => Some(Self::PrettyJson),
            "xform_minify_json" => Some(Self::MinifyJson),
            "xform_json_yaml" => Some(Self::JsonToYaml),
            "xform_yaml_json" => Some(Self::YamlToJson),
            _ => None,
        }
    }
}

pub fn apply(action: Transform, text: &str) -> Result<String, String> {
    match action {
        Transform::PrettyJson => pretty_json(text),
        Transform::MinifyJson => minify_json(text),
        Transform::JsonToYaml => json_to_yaml(text),
        Transform::YamlToJson => yaml_to_json(text),
    }
}

pub fn pretty_json(text: &str) -> Result<String, String> {
    let value = parse_json(text)?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

pub fn minify_json(text: &str) -> Result<String, String> {
    let value = parse_json(text)?;
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

pub fn json_to_yaml(text: &str) -> Result<String, String> {
    let value = parse_json(text)?;
    serde_yaml::to_string(&value).map_err(|e| e.to_string())
}

pub fn yaml_to_json(text: &str) -> Result<String, String> {
    let value = parse_yaml(text)?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
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
    use super::{Transform, apply, json_to_yaml, looks_like_yaml, minify_json, yaml_to_json};

    #[test]
    fn minifies_json() {
        let out = minify_json("{\n  \"a\": 1\n}").unwrap();
        assert_eq!(out, "{\"a\":1}");
    }

    #[test]
    fn json_roundtrip_yaml() {
        let yaml = json_to_yaml("{\"name\":\"copycraft\"}").unwrap();
        assert!(yaml.contains("name"));
        assert!(yaml.contains("copycraft"));
        let json = yaml_to_json(&yaml).unwrap();
        assert!(json.contains("copycraft"));
    }

    #[test]
    fn detects_yaml_not_json() {
        assert!(looks_like_yaml("name: copycraft\nitems:\n  - one\n"));
        assert!(!looks_like_yaml("{\"a\":1}"));
    }

    #[test]
    fn apply_pretty_rejects_plain_text() {
        assert!(apply(Transform::PrettyJson, "hello").is_err());
    }
}
