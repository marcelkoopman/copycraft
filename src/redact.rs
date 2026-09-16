use std::sync::{Arc, OnceLock};

use redact_core::recognizers::pattern::PatternRecognizer;
use redact_core::recognizers::Recognizer;
use redact_core::types::RecognizerResult;
use redact_core::{
    AnalyzerEngine, AnonymizationStrategy, AnonymizerConfig, EntityType,
};

pub fn redact(text: &str) -> String {
    let config = AnonymizerConfig {
        strategy: AnonymizationStrategy::Replace,
        ..Default::default()
    };
    engine()
        .anonymize(text, None, &config)
        .map(|result| result.text)
        .unwrap_or_else(|_| text.to_string())
}

fn engine() -> &'static AnalyzerEngine {
    static ENGINE: OnceLock<AnalyzerEngine> = OnceLock::new();
    ENGINE.get_or_init(build_engine)
}

fn build_engine() -> AnalyzerEngine {
    let mut engine = AnalyzerEngine::new();
    engine
        .recognizer_registry_mut()
        .add_recognizer(Arc::new(LabeledFieldRecognizer));
    engine
        .recognizer_registry_mut()
        .add_recognizer(Arc::new(dutch_phone_recognizer()));
    engine
}

fn dutch_phone_recognizer() -> PatternRecognizer {
    let mut recognizer = PatternRecognizer::with_name("nl-phone");
    let _ = recognizer.add_pattern_with_context(
        EntityType::PhoneNumber,
        r"(?i)(?:\+31[\s\-]?6|06)[\s\-]?\d{8}\b|(?:\+31[\s\-]?6|06)[\s\-]?\d{2}[\s\-]?\d{6}\b|(?:\+31[\s\-]?6|06)[\s\-]?\d{2}[\s\-]?\d{2}[\s\-]?\d{2}[\s\-]?\d{2}\b",
        0.85,
        vec![
            "telefoon".into(),
            "telefoonnummer".into(),
            "mobiel".into(),
            "phone".into(),
        ],
    );
    recognizer
}

#[derive(Debug)]
struct LabeledFieldRecognizer;

impl Recognizer for LabeledFieldRecognizer {
    fn name(&self) -> &str {
        "labeled-field"
    }

    fn supported_entities(&self) -> &[EntityType] {
        &[
            EntityType::Person,
            EntityType::Location,
            EntityType::DateTime,
            EntityType::EmailAddress,
            EntityType::PhoneNumber,
        ]
    }

    fn supports_language(&self, _language: &str) -> bool {
        true
    }

    fn analyze(
        &self,
        text: &str,
        _language: &str,
    ) -> anyhow::Result<Vec<RecognizerResult>> {
        let mut results = Vec::new();
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let content = line.trim_end_matches(['\n', '\r']);
            if let Some((label, value_start_in_line)) = labeled_value_start(content) {
                let Some(entity) = entity_for_label(label) else {
                    offset += line.len();
                    continue;
                };
                let value_start = offset + value_start_in_line;
                let value_end = offset + content.len();
                if value_end > value_start {
                    results.push(
                        RecognizerResult::new(entity, value_start, value_end, 0.95, self.name())
                            .with_text(text),
                    );
                }
            }
            offset += line.len();
        }
        Ok(results)
    }
}

fn labeled_value_start(line: &str) -> Option<(&str, usize)> {
    let colon = line.find(':')?;
    let label = line[..colon].trim();
    if label.is_empty() || label.chars().count() > 40 {
        return None;
    }
    if entity_for_label(label).is_none() {
        return None;
    }
    let after = &line[colon + 1..];
    let value = after.trim_start();
    if value.is_empty() {
        return None;
    }
    let value_start = colon + 1 + (after.len() - value.len());
    Some((label, value_start))
}

fn entity_for_label(label: &str) -> Option<EntityType> {
    let key = label
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| *c != '.')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(match key.as_str() {
        "naam" | "name" | "voornaam" | "achternaam" | "full name" => EntityType::Person,
        "adres" | "address" | "straat" | "woonplaats" | "city" => EntityType::Location,
        "geboortedatum" | "geboorte datum" | "date of birth" | "dob" | "geboorte" => {
            EntityType::DateTime
        }
        "salaris" | "salary" | "inkomen" | "bedrag" | "amount" => {
            EntityType::Custom("AMOUNT".into())
        }
        "e-mailadres" | "emailadres" | "e-mail" | "email" | "mail" => EntityType::EmailAddress,
        "telefoonnummer" | "telefoon" | "phone" | "phonenumber" | "mobiel" => {
            EntityType::PhoneNumber
        }
        "bsn" | "sofinummer" => EntityType::Custom("NL_BSN".into()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_email() {
        let out = redact("mail me at jan.devries@email.nl please");
        assert!(!out.contains("jan.devries@email.nl"));
    }

    #[test]
    fn redacts_phone_and_url() {
        let out = redact("call 555-123-4567 see https://example.com/x");
        assert!(!out.contains("555-123-4567"));
        assert!(!out.contains("https://example.com/x"));
    }

    #[test]
    fn redacts_dutch_mobile() {
        let out = redact("Telefoonnummer: 06-12345678");
        assert!(!out.contains("06-12345678"));
        assert!(out.contains("Telefoonnummer:"));
    }

    #[test]
    fn redacts_labeled_dutch_record() {
        let src = "\
Naam: Jan de Vries

Adres: Hoofdstraat 45, 9711 AB Groningen

E-mailadres: jan.devries@email.nl

Telefoonnummer: 06-12345678

Geboortedatum: 12 mei 1984

Salaris: € 3.450";
        let out = redact(src);
        assert!(out.contains("Naam:"));
        assert!(out.contains("Adres:"));
        assert!(out.contains("E-mailadres:"));
        assert!(out.contains("Telefoonnummer:"));
        assert!(out.contains("Geboortedatum:"));
        assert!(out.contains("Salaris:"));
        assert!(!out.contains("Jan de Vries"));
        assert!(!out.contains("Hoofdstraat"));
        assert!(!out.contains("jan.devries@email.nl"));
        assert!(!out.contains("06-12345678"));
        assert!(!out.contains("12 mei 1984"));
        assert!(!out.contains("3.450"));
        assert_eq!(out.lines().filter(|l| l.contains(':')).count(), 6);
    }

    #[test]
    fn redacts_iban() {
        let out = redact("pay to NL91ABNA0417164300");
        assert!(!out.contains("NL91ABNA0417164300"));
    }
}
