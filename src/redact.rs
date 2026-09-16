use std::sync::{Arc, OnceLock};

use redact_core::recognizers::pattern::PatternRecognizer;
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
    engine
        .recognizer_registry_mut()
        .add_recognizer(Arc::new(dutch_phones));
    engine
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
    fn keeps_labeled_record_structure() {
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
        assert!(!out.contains("jan.devries@email.nl"));
        assert!(!out.contains("06-12345678"));
        assert_eq!(out.lines().filter(|l| l.contains(':')).count(), 6);
    }

    #[test]
    fn redacts_iban() {
        let out = redact("pay to NL91ABNA0417164300");
        assert!(!out.contains("NL91ABNA0417164300"));
    }
}
