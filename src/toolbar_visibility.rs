use crate::format::FormatKind;

pub fn shows_redact(kind: FormatKind) -> bool {
    matches!(
        kind,
        FormatKind::Dataframe | FormatKind::Text | FormatKind::Plain
    )
}

pub fn shows_dataframe(kind: FormatKind) -> bool {
    matches!(
        kind,
        FormatKind::Json | FormatKind::Xml | FormatKind::Dataframe
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

#[cfg(test)]
mod tests {
    use super::{shows_compress, shows_dataframe, shows_redact};
    use crate::format::FormatKind;

    #[test]
    fn code_and_config_hide_redact_and_dataframe() {
        for kind in [
            FormatKind::Rust,
            FormatKind::Java,
            FormatKind::Yaml,
            FormatKind::Url,
        ] {
            assert!(!shows_redact(kind), "{kind:?}");
            assert!(!shows_dataframe(kind), "{kind:?}");
            assert!(shows_compress(kind), "{kind:?}");
        }
    }

    #[test]
    fn json_hides_redact_keeps_dataframe() {
        assert!(!shows_redact(FormatKind::Json));
        assert!(shows_dataframe(FormatKind::Json));
        assert!(!shows_compress(FormatKind::Json));
    }

    #[test]
    fn xml_dataframe_without_redact() {
        assert!(!shows_redact(FormatKind::Xml));
        assert!(shows_dataframe(FormatKind::Xml));
        assert!(!shows_compress(FormatKind::Xml));
    }

    #[test]
    fn table_and_prose() {
        assert!(shows_redact(FormatKind::Dataframe));
        assert!(shows_dataframe(FormatKind::Dataframe));
        assert!(!shows_compress(FormatKind::Dataframe));
        assert!(shows_redact(FormatKind::Text));
        assert!(!shows_dataframe(FormatKind::Text));
        assert!(shows_compress(FormatKind::Text));
    }
}
