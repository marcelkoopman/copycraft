use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

pub fn pretty_yaml(text: &str) -> Result<String, String> {
    let value = parse_yaml(text)?;
    serde_yaml::to_string(&value).map_err(|e| e.to_string())
}

pub fn looks_like_yaml(text: &str) -> bool {
    if looks_like_labeled_record(text) {
        return false;
    }
    parse_yaml(text).is_ok() && parse_json(text).is_err()
}

fn looks_like_labeled_record(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < 2 {
        return false;
    }
    let labeled = lines
        .iter()
        .filter(|line| line_is_label_value(line))
        .count();
    labeled * 2 >= lines.len()
}

fn line_is_label_value(line: &str) -> bool {
    let Some((label, value)) = line.split_once(':') else {
        return false;
    };
    let label = label.trim();
    let value = value.trim();
    !label.is_empty()
        && !value.is_empty()
        && !label.starts_with('-')
        && !label.contains('{')
        && label.chars().count() <= 40
        && !label.chars().any(|c| c == '[' || c == ']')
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

    #[test]
    fn labeled_personal_record_is_not_yaml() {
        let src = "Naam: [PERSON]
Adres: [LOCATION]
E-mailadres: [EMAIL_ADDRESS]
Telefoonnummer: [PHONE_NUMBER]
Geboortedatum: [DATE_TIME]
Salaris: [MONEY]";
        assert!(!looks_like_yaml(src));
    }

    #[test]
    fn blank_line_labeled_record_is_not_yaml() {
        let src = "Naam: Jan de Vries

Adres: Hoofdstraat 45, 9711 AB Groningen

E-mailadres: jan.devries@email.nl

Telefoonnummer: 06-12345678

Geboortedatum: 12 mei 1984

Salaris: € 3.450";
        assert!(!looks_like_yaml(src));
    }
}
