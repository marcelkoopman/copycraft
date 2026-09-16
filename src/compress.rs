use compression_prompt::{Compressor, CompressorConfig};

pub fn try_compress(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.len() < 80 {
        return None;
    }
    let compressor = Compressor::new(CompressorConfig::default());
    let result = compressor.compress(text).ok()?;
    if result.compressed.trim().is_empty() || result.compressed == text {
        return None;
    }
    if result.compressed_tokens >= result.original_tokens {
        return None;
    }
    Some(result.compressed)
}

#[cfg(test)]
mod tests {
    use super::try_compress;

    #[test]
    fn short_input_is_not_compressed() {
        assert!(try_compress("hi there").is_none());
    }

    #[test]
    fn long_prose_shrinks_or_stays_readable() {
        let src = "Please summarize the following meeting notes for the team. "
            .repeat(20);
        if let Some(out) = try_compress(&src) {
            assert!(!out.is_empty());
            assert!(out.len() <= src.len());
        }
    }
}
