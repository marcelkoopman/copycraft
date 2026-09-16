use compression_prompt::{Compressor, CompressorConfig};

pub const MIN_COMPRESS_BYTES: usize = 1024;

pub fn is_large_enough(text: &str) -> bool {
    text.len() >= MIN_COMPRESS_BYTES
}

pub fn try_compress(text: &str) -> Option<String> {
    if !is_large_enough(text) {
        return None;
    }
    let config = CompressorConfig {
        target_ratio: 0.5,
        min_input_tokens: 1,
        min_input_bytes: MIN_COMPRESS_BYTES,
    };
    let result = Compressor::new(config).compress(text).ok()?;
    if result.compressed.trim().is_empty() {
        return None;
    }
    Some(result.compressed)
}

#[cfg(test)]
mod tests {
    use super::{is_large_enough, try_compress};

    #[test]
    fn short_text_is_not_large_enough() {
        assert!(!is_large_enough("i cannot click on the buttons compress"));
        assert!(try_compress("i cannot click on the buttons compress").is_none());
    }

    #[test]
    fn kilobyte_text_is_large_enough() {
        let src = "please summarize these notes for the team. ".repeat(40);
        assert!(is_large_enough(&src));
    }
}
