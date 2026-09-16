use std::sync::{Arc, OnceLock};

use anyhow::Result;
use redact_core::recognizers::pattern::PatternRecognizer;
use redact_core::recognizers::Recognizer;
use redact_core::{
    AnalyzerEngine, AnonymizationStrategy, AnonymizerConfig, EntityType, RecognizerResult,
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
        .add_recognizer(Arc::new(dutch_phone_recognizer()));
    engine
        .recognizer_registry_mut()
        .add_recognizer(Arc::new(LabeledFieldRecognizer::new()));
    engine
}

fn dutch_phone_recognizer() -> PatternRecognizer {
    let mut dutch_phones = PatternRecognizer::with_name("nl-phone");
    let _ = dutch_phones.add_pattern_with_context(
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
    dutch_phones
}

#[derive(Debug)]
struct LabeledFieldRecognizer {
    supported: Vec<EntityType>,
}

impl LabeledFieldRecognizer {
    fn new() -> Self {
        Self {
            supported: vec![
                EntityType::Person,
                EntityType::Location,
                EntityType::DateTime,
                EntityType::Custom("AMOUNT".into()),
                EntityType::Custom("NL_BSN".into()),
            ],
        }
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
        match key.as_str() {
            "naam" | "name" | "voornaam" | "achternaam" | "full name" => Some(EntityType::Person),
            "adres" | "address" | "straat" | "woonplaats" => Some(EntityType::Location),
            "geboortedatum" | "geboorte datum" | "date of birth" | "dob" | "birthdate" => {
                Some(EntityType::DateTime)
            }
            "salaris" | "salary" | "inkomen" | "loon" => Some(EntityType::Custom("AMOUNT".into())),
            "bsn" | "sofinummer" => Some(EntityType::Custom("NL_BSN".into())),
            _ => None,
        }
    }
}

impl Recognizer for LabeledFieldRecognizer {
    fn name(&self) -> &str {
        "labeled-fields"
    }

    fn supported_entities(&self) -> &[EntityType] {
        &self.supported
    }

    fn supports_language(&self, _language: &str) -> bool {
        true
    }

    fn analyze(&self, text: &str, _language: &str) -> Result<Vec<RecognizerResult>> {
        let mut results = Vec::new();
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let content = line.trim_end_matches(['\r', '\n']);
            if let Some(colon) = content.find(':') {
                let label = content[..colon].trim();
                let value = content[colon + 1..].trim();
                if !label.is_empty()
                    && !value.is_empty()
                    && label.chars().count() <= 40
                    && let Some(entity) = Self::entity_for_label(label)
                {
                    let value_start_in_line = content[colon + 1..]
                        .find(value)
                        .map(|i| colon + 1 + i)
                        .unwrap_or(colon + 1);
                    let start = offset + value_start_in_line;
                    let end = start + value.len();
                    results.push(
                        RecognizerResult::new(entity, start, end, 0.9, self.name()).with_text(text),
                    );
                }
            }
            offset += line.len();
        }
        Ok(results)
    }
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
