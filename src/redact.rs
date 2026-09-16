use std::sync::{Arc, OnceLock};

use redact_core::recognizers::Recognizer;
use redact_core::recognizers::pattern::PatternRecognizer;
use redact_core::types::RecognizerResult;
use redact_core::{AnalyzerEngine, AnonymizationStrategy, AnonymizerConfig, EntityType};

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
        .add_recognizer(Arc::new(TabularFieldRecognizer));
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

    fn analyze(&self, text: &str, _language: &str) -> anyhow::Result<Vec<RecognizerResult>> {
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

#[derive(Debug)]
struct TabularFieldRecognizer;

impl Recognizer for TabularFieldRecognizer {
    fn name(&self) -> &str {
        "tabular-field"
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

    fn analyze(&self, text: &str, _language: &str) -> anyhow::Result<Vec<RecognizerResult>> {
        let Some(table) = parse_pii_table(text) else {
            return Ok(Vec::new());
        };
        let mut results = Vec::new();
        let mut offset = 0usize;
        let mut row_index = 0usize;
        for line in text.split_inclusive('\n') {
            let content = line.trim_end_matches(['\n', '\r']);
            if content.trim().is_empty() {
                offset += line.len();
                continue;
            }
            if row_index > 0 {
                for (col, entity) in &table.pii_columns {
                    if let Some((start_in_line, end_in_line)) =
                        cell_span(content, table.delimiter, *col)
                    {
                        let value_start = offset + start_in_line;
                        let value_end = offset + end_in_line;
                        if value_end > value_start {
                            results.push(
                                RecognizerResult::new(
                                    entity.clone(),
                                    value_start,
                                    value_end,
                                    0.95,
                                    self.name(),
                                )
                                .with_text(text),
                            );
                        }
                    }
                }
            }
            row_index += 1;
            offset += line.len();
        }
        Ok(results)
    }
}

struct PiiTable {
    delimiter: char,
    pii_columns: Vec<(usize, EntityType)>,
}

fn parse_pii_table(text: &str) -> Option<PiiTable> {
    let header = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let delimiter = detect_delimiter(header)?;
    let headers: Vec<&str> = header.split(delimiter).map(str::trim).collect();
    if headers.len() < 2 {
        return None;
    }
    let pii_columns: Vec<(usize, EntityType)> = headers
        .iter()
        .enumerate()
        .filter_map(|(index, label)| entity_for_label(label).map(|entity| (index, entity)))
        .collect();
    if pii_columns.is_empty() {
        return None;
    }
    Some(PiiTable {
        delimiter,
        pii_columns,
    })
}

fn detect_delimiter(header: &str) -> Option<char> {
    let semis = header.matches(';').count();
    let commas = header.matches(',').count();
    if semis >= 1 && semis >= commas {
        Some(';')
    } else if commas >= 1 {
        Some(',')
    } else {
        None
    }
}

fn cell_span(line: &str, delimiter: char, column: usize) -> Option<(usize, usize)> {
    let mut start = 0usize;
    let mut index = 0usize;
    for (idx, ch) in line.char_indices() {
        if ch == delimiter {
            if index == column {
                return trim_cell_span(line, start, idx);
            }
            start = idx + ch.len_utf8();
            index += 1;
        }
    }
    if index == column {
        return trim_cell_span(line, start, line.len());
    }
    None
}

fn trim_cell_span(line: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let cell = &line[start..end];
    let trimmed = cell.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lead = cell.len() - cell.trim_start().len();
    let trail = cell.len() - cell.trim_end().len();
    Some((start + lead, end - trail))
}

fn labeled_value_start(line: &str) -> Option<(&str, usize)> {
    let colon = line.find(':')?;
    let label = line[..colon].trim();
    if label.is_empty() || label.chars().count() > 40 {
        return None;
    }
    entity_for_label(label)?;
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
    fn redacts_semicolon_csv_naam_and_salaris() {
        let src = "\
Id;Naam;Geboortedatum;Adres;Telefoonnummer;Salaris
1;Jan de Vries;12-05-1984;Hoofdstraat 45, Amsterdam;06-12345678;3450
2;Anja Bakker;03-11-1990;Kerkplein 2, Utrecht;06-87654321;2900
3;Mohammed El Amin;21-02-1978;Stationstraat 120, Rotterdam;06-11223344;4200";
        let out = redact(src);
        assert!(out.lines().next().unwrap().contains("Naam"));
        assert!(out.lines().next().unwrap().contains("Salaris"));
        assert!(!out.contains("Jan de Vries"));
        assert!(!out.contains("Anja Bakker"));
        assert!(!out.contains("Mohammed El Amin"));
        assert!(!out.contains(";3450"));
        assert!(!out.contains(";2900"));
        assert!(!out.contains(";4200"));
        assert!(out.contains("[PERSON]"));
        assert!(out.contains("[AMOUNT]"));
    }

    #[test]
    fn redacts_iban() {
        let out = redact("pay to NL91ABNA0417164300");
        assert!(!out.contains("NL91ABNA0417164300"));
    }
}
