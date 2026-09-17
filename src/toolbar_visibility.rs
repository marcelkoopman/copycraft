use crate::format::FormatKind;

pub fn shows_format(source: &str) -> bool {
    crate::format::format_text(source) != source
}

pub fn shows_redact(kind: FormatKind, source: &str) -> bool {
    matches!(
        kind,
        FormatKind::Csv
            | FormatKind::Tsv
            | FormatKind::Dataframe
            | FormatKind::Text
            | FormatKind::Plain
    ) && crate::redact::redact(source) != source
}

pub fn shows_dataframe(kind: FormatKind) -> bool {
    matches!(
        kind,
        FormatKind::Json
            | FormatKind::Xml
            | FormatKind::Csv
            | FormatKind::Tsv
            | FormatKind::Dataframe
    )
}

pub fn shows_compress(kind: FormatKind) -> bool {
    matches!(
        kind,
        FormatKind::Text
            | FormatKind::Plain
            | FormatKind::Rust
            | FormatKind::Java
            | FormatKind::Yaml
            | FormatKind::Url
    )
}

pub fn shows_decode(kind: FormatKind) -> bool {
    matches!(kind, FormatKind::Plain | FormatKind::Text | FormatKind::Url)
}

#[cfg(test)]
mod tests {
    use super::{shows_compress, shows_dataframe, shows_decode, shows_format, shows_redact};
    use crate::format::FormatKind;

    #[test]
    fn code_and_config_hide_redact_and_dataframe() {
        for kind in [
            FormatKind::Rust,
            FormatKind::Java,
            FormatKind::Yaml,
            FormatKind::Url,
        ] {
            assert!(
                !shows_redact(kind, "mail me at jan@example.com"),
                "{kind:?}"
            );
            assert!(!shows_dataframe(kind), "{kind:?}");
            assert!(shows_compress(kind), "{kind:?}");
        }
        assert!(!shows_decode(FormatKind::Rust));
        assert!(!shows_decode(FormatKind::Java));
        assert!(!shows_decode(FormatKind::Yaml));
        assert!(shows_decode(FormatKind::Url));
    }

    #[test]
    fn json_hides_redact_keeps_dataframe() {
        assert!(!shows_redact(
            FormatKind::Json,
            r#"{"email":"jan.devries@email.nl"}"#
        ));
        assert!(shows_dataframe(FormatKind::Json));
        assert!(!shows_compress(FormatKind::Json));
        assert!(!shows_decode(FormatKind::Json));
    }

    #[test]
    fn xml_dataframe_without_redact() {
        assert!(!shows_redact(
            FormatKind::Xml,
            "<root><email>jan.devries@email.nl</email></root>"
        ));
        assert!(shows_dataframe(FormatKind::Xml));
        assert!(!shows_compress(FormatKind::Xml));
        assert!(!shows_decode(FormatKind::Xml));
    }

    #[test]
    fn table_and_prose() {
        let pii = "mail me at jan.devries@email.nl please";
        assert!(shows_redact(FormatKind::Dataframe, pii));
        assert!(shows_dataframe(FormatKind::Dataframe));
        assert!(!shows_compress(FormatKind::Dataframe));
        assert!(!shows_decode(FormatKind::Dataframe));
        assert!(shows_redact(FormatKind::Text, pii));
        assert!(!shows_dataframe(FormatKind::Text));
        assert!(shows_compress(FormatKind::Text));
        assert!(shows_decode(FormatKind::Text));
        assert!(shows_decode(FormatKind::Plain));
        assert!(shows_dataframe(FormatKind::Csv));
        assert!(shows_dataframe(FormatKind::Tsv));
        assert!(shows_redact(FormatKind::Csv, pii));
        assert!(shows_redact(FormatKind::Tsv, pii));
        assert!(!shows_compress(FormatKind::Csv));
        assert!(!shows_compress(FormatKind::Tsv));
        assert!(!shows_decode(FormatKind::Csv));
        assert!(!shows_decode(FormatKind::Tsv));
    }

    #[test]
    fn redact_hides_when_output_matches_source() {
        assert!(!shows_redact(FormatKind::Plain, "hello world"));
        assert!(!shows_redact(FormatKind::Text, "line one\nline two"));
        assert!(!shows_redact(
            FormatKind::Dataframe,
            "col_a,col_b\n1,2\n3,4"
        ));
    }

    #[test]
    fn redact_shows_when_sensitive_values_change() {
        assert!(shows_redact(
            FormatKind::Plain,
            "mail me at jan.devries@email.nl please"
        ));
        assert!(shows_redact(
            FormatKind::Text,
            "Telefoonnummer: 06-12345678"
        ));
    }

    #[test]
    fn format_hides_when_output_matches_source() {
        assert!(!shows_format("hello world"));
        assert!(!shows_format("line one\nline two"));
        assert!(!shows_format("https://example.com/path"));
        assert!(!shows_format("name,age\nalice,30\nbob,40"));
        assert!(!shows_format("name\tage\nalice\t30\nbob\t40"));
        let pretty = "{\n  \"name\": \"copycraft\"\n}";
        assert!(!shows_format(pretty));
    }

    #[test]
    fn format_shows_when_pretty_print_changes_text() {
        assert!(shows_format(r#"{"name":"copycraft"}"#));
        assert!(shows_format("<root><item/></root>"));
    }
}
