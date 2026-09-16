use compression_prompt::{Compressor, CompressorConfig};

pub fn try_compress(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let config = CompressorConfig {
        target_ratio: 0.5,
        min_input_tokens: 1,
        min_input_bytes: 1,
    };
    if let Ok(result) = Compressor::new(config).compress(text) {
        if !result.compressed.trim().is_empty() {
            return Some(result.compressed);
        }
    }
    Some(compact_prompt(trimmed))
}

fn compact_prompt(text: &str) -> String {
    const DROP: &[&str] = &[
        "a", "an", "the", "i", "i'm", "im", "just", "really", "very", "please",
        "kind", "of", "sort", "that", "this", "those", "these",
    ];
    text.split_whitespace()
        .filter(|word| {
            let key = word.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase();
            !DROP.contains(&key.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::try_compress;

    #[test]
    fn empty_is_none() {
        assert!(try_compress("   ").is_none());
    }

    #[test]
    fn short_sentence_compresses() {
        let out = try_compress("i cannot click on the buttons compress").expect("short");
        assert!(!out.is_empty());
        assert!(out.to_ascii_lowercase().contains("compress"));
        assert!(out.len() <= "i cannot click on the buttons compress".len());
    }
}
